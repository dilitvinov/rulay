use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use hkdf::Hkdf;
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};

pub fn parse_key_share_bytes(buf: &[u8]) -> Result<[u8; 32], &'static str> {
    if buf.is_empty() || buf[0] != 0x16 {
        return Err("not a TLS handshake record (expected 0x16)");
    }

    // TLS record: content_type(1) + version(2) + length(2) = 5
    // Handshake:  msg_type(1) + length(3) = 4
    // ClientHello: client_version(2) + random(32) = 34
    // Total fixed prefix before session_id_len: 5 + 4 + 34 = 43
    let mut pos = 43;

    if buf.len() < pos + 1 {
        return Err("too short: session_id");
    }
    let sid_len = buf[pos] as usize;
    pos += 1 + sid_len;

    if buf.len() < pos + 2 {
        return Err("too short: cipher_suites");
    }
    let cs_len = u16::from_be_bytes([buf[pos], buf[pos + 1]]) as usize;
    pos += 2 + cs_len;

    if buf.len() < pos + 1 {
        return Err("too short: compression_methods");
    }
    let cm_len = buf[pos] as usize;
    pos += 1 + cm_len;

    if buf.len() < pos + 2 {
        return Err("too short: extensions_len");
    }
    let ext_end = pos + 2 + u16::from_be_bytes([buf[pos], buf[pos + 1]]) as usize;
    pos += 2;

    while pos + 4 <= ext_end && pos + 4 <= buf.len() {
        let ext_type = u16::from_be_bytes([buf[pos], buf[pos + 1]]);
        let ext_len = u16::from_be_bytes([buf[pos + 2], buf[pos + 3]]) as usize;
        pos += 4;

        if ext_type == 0x0033 {
            if buf.len() < pos + 2 {
                return Err("key_share: too short");
            }
            let shares_len = u16::from_be_bytes([buf[pos], buf[pos + 1]]) as usize;
            let mut kpos = pos + 2;
            let shares_end = kpos + shares_len;

            // iterate entries: Chrome puts a GREASE entry before X25519
            while kpos + 4 <= shares_end && kpos + 4 <= buf.len() {
                let group = u16::from_be_bytes([buf[kpos], buf[kpos + 1]]);
                let kx_len = u16::from_be_bytes([buf[kpos + 2], buf[kpos + 3]]) as usize;
                kpos += 4;

                if group == 0x001D {
                    if buf.len() < kpos + kx_len || kx_len != 32 {
                        return Err("key_share: X25519 key truncated or wrong size");
                    }
                    return Ok(buf[kpos..kpos + 32].try_into().unwrap());
                }

                kpos += kx_len;
            }

            return Err("X25519 entry not found in key_share");
        }

        pos += ext_len;
    }

    Err("key_share extension not found")
}

// Reality auth (Xray-core algorithm):
//
//   shared   = X25519(server_priv, client_eph_pub)
//   auth_key = HKDF-SHA256(ikm=shared, salt=client_random[:20], info="REALITY") → 32 bytes
//
//   client encrypted session_id as:
//     AES-256-GCM(key=auth_key, nonce=client_random[20:], plaintext=session_id_plain[:16],
//                 aad=hello_raw_with_session_id_zeroed)
//
//   server verifies by decrypting — successful AEAD open = authentic client
//
pub fn verify_reality_auth(buf: &[u8], server_priv_b64: &str) -> Result<bool, &'static str> {
    if buf.len() < 76 {
        return Err("buf too short");
    }
    if buf[0] != 0x16 {
        return Err("not a TLS handshake record");
    }

    let record_len = u16::from_be_bytes([buf[3], buf[4]]) as usize;
    if buf.len() < 5 + record_len {
        return Err("buf truncated: record_len exceeds buffer");
    }

    // client_random: buf[11..43] (32 bytes)
    let client_random = &buf[11..43];
    let hkdf_salt = &client_random[..20]; // → HKDF salt
    let gcm_nonce = &client_random[20..]; // → AES-GCM nonce (12 bytes)

    // session_id: buf[43] = len, buf[44..44+len] = ciphertext (16 bytes plaintext + 16 bytes GCM tag = 32)
    let sid_len = buf[43] as usize;
    if sid_len != 32 {
        return Ok(false);
    }
    if buf.len() < 44 + 32 {
        return Err("buf too short: session_id");
    }
    let session_id_ct = &buf[44..76];

    let client_pub_bytes = parse_key_share_bytes(buf)?;

    let server_priv_bytes: [u8; 32] = URL_SAFE_NO_PAD
        .decode(server_priv_b64)
        .map_err(|_| "server priv: bad base64")?
        .try_into()
        .map_err(|_| "server priv: not 32 bytes")?;

    let shared = StaticSecret::from(server_priv_bytes)
        .diffie_hellman(&PublicKey::from(client_pub_bytes));

    // HKDF-SHA256(ikm=shared, salt=client_random[:20], info="REALITY") → 32 bytes
    let hkdf = Hkdf::<Sha256>::new(Some(hkdf_salt), shared.as_bytes());
    let mut auth_key = [0u8; 32];
    hkdf.expand(b"REALITY", &mut auth_key)
        .map_err(|_| "hkdf expand failed")?;

    // AAD = hello.Raw (buf[5..5+record_len]) with session_id bytes zeroed.
    // Within hello.Raw the session_id lives at offset 39..71:
    //   Handshake header(4) + client_version(2) + random(32) + session_id_len(1) = 39
    let mut aad = buf[5..5 + record_len].to_vec();
    aad[39..71].fill(0);

    let key = Key::<Aes256Gcm>::from_slice(&auth_key);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(gcm_nonce);

    match cipher.decrypt(nonce, Payload { msg: session_id_ct, aad: &aad }) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// Builds a minimal TLS ClientHello whose session_id is REALITY-authenticated for `server_priv_b64`
/// (the client side of `verify_reality_auth`).
#[cfg(test)]
pub fn reality_client_hello(server_priv_b64: &str) -> Vec<u8> {
    let server_priv: [u8; 32] = URL_SAFE_NO_PAD.decode(server_priv_b64).unwrap().try_into().unwrap();
    let server_pub = PublicKey::from(&StaticSecret::from(server_priv));
    let client_secret = StaticSecret::from([7u8; 32]);
    let client_pub = PublicKey::from(&client_secret);
    let random: [u8; 32] = std::array::from_fn(|i| i as u8);

    // key_share extension with a single X25519 entry
    let mut ext = Vec::new();
    ext.extend_from_slice(&0x0033u16.to_be_bytes());
    ext.extend_from_slice(&(2u16 + 4 + 32).to_be_bytes());
    ext.extend_from_slice(&(4u16 + 32).to_be_bytes());
    ext.extend_from_slice(&0x001Du16.to_be_bytes());
    ext.extend_from_slice(&32u16.to_be_bytes());
    ext.extend_from_slice(client_pub.as_bytes());

    let mut body = vec![0x03, 0x03];
    body.extend_from_slice(&random);
    body.push(32);
    body.extend_from_slice(&[0u8; 32]); // session_id, filled in below
    body.extend_from_slice(&[0x00, 0x02, 0x13, 0x01]); // cipher_suites: TLS_AES_128_GCM_SHA256
    body.extend_from_slice(&[0x01, 0x00]); // compression_methods: null
    body.extend_from_slice(&(ext.len() as u16).to_be_bytes());
    body.extend_from_slice(&ext);

    // hello.Raw = handshake header + body; the AAD is this with the session_id still zeroed
    let mut raw = vec![0x01];
    raw.extend_from_slice(&(body.len() as u32).to_be_bytes()[1..]);
    raw.extend_from_slice(&body);

    let shared = client_secret.diffie_hellman(&server_pub);
    let mut auth_key = [0u8; 32];
    Hkdf::<Sha256>::new(Some(&random[..20]), shared.as_bytes())
        .expand(b"REALITY", &mut auth_key)
        .unwrap();
    let session_id = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&auth_key))
        .encrypt(Nonce::from_slice(&random[20..]), Payload { msg: &[0u8; 16], aad: &raw })
        .unwrap();
    raw[39..71].copy_from_slice(&session_id);

    let mut record = vec![0x16, 0x03, 0x01];
    record.extend_from_slice(&(raw.len() as u16).to_be_bytes());
    record.extend_from_slice(&raw);
    record
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY_A: &str = "uM5Zol5nBgyqDrn2RYGhmTeoONiULxeLMhkeDqMtMUE";
    const KEY_B: &str = "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE";

    #[test]
    fn accepts_hello_for_own_key() {
        assert_eq!(verify_reality_auth(&reality_client_hello(KEY_A), KEY_A), Ok(true));
    }

    #[test]
    fn rejects_hello_for_other_key() {
        assert_eq!(verify_reality_auth(&reality_client_hello(KEY_B), KEY_A), Ok(false));
    }
}
