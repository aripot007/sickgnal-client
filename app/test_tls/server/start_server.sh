#!/bin/bash

openssl s_server -rev -key server_key.pem -cert server_cert.pem -accept 4267 -keylogfile session_keys.log -security_debug_verbose "$@"
