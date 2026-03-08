#!/bin/bash

# CA
openssl req -x509 -newkey rsa:2048 -nodes \
  -keyout ca_key.pem -out ca_cert.pem -days 365 \
  -subj "/CN=localhost CA"

# Server

openssl genrsa -out server_key.pem 2048

openssl req -new -key server_key.pem -out server.csr \
  -subj "/CN=localhost"

cat > san.ext << EOF
authorityKeyIdentifier=keyid,issuer
basicConstraints=CA:FALSE
keyUsage = digitalSignature, nonRepudiation, keyEncipherment, dataEncipherment
subjectAltName = DNS:localhost, IP:127.0.0.1
EOF

openssl x509 -req -in server.csr -CA ca_cert.pem -CAkey ca_key.pem \
  -out server_cert.pem -days 365 -sha256 -extfile san.ext

rm server.csr san.ext
