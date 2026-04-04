./install.sh --build-only

./install.sh --mode transmitter --upstream-addr 0.0.0.0:8444
--downstream-addr 0.0.0.0:8443

./install.sh --mode receiver --upstream-addr 1.2.3.4:8443
--downstream-addr 5.6.7.8:8444
