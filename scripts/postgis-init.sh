#!/bin/sh
SSL_DIR=/ssl
CERT_DIR=${SSL_DIR}/certs
KEY_DIR=${SSL_DIR}/keys
CLIENT=svc_gis

set -e

if [ ! -f "/.dockerenv" ]; then
	printf "%s\n" "This script is meant as init-script for postgis in docker. Refusing to run here."
	exit 0
fi

# Do we have expected volume/mounts? Create dir if nothing's there.
for d in "${SSL_DIR}" "${CERT_DIR}" "${KEY_DIR}"
do
	if [ ! -d "${d}" ]; then
		if [ ! -e "${d}" ]; then
			printf "%s\n"  "Creating non-existing dir ${d}"
			mkdir -p "${d}"
		else
			printf "%s\n" "SSL related directory expected at ${d} is not a dir. Refusing."
			exit 1
		fi
	fi
done

# Debug
printf "Using SSL dirs:\nSSL base:\t%s\nCert dir:\t%s\nKey dir:\t%s\n\n" "${SSL_DIR}" "${CERT_DIR}" "${KEY_DIR}"

# Root cert
if [ ! -f "${CERT_DIR}/root.crt" ]; then
	printf "%s\n" "Creating root certificate request...."

	# Create Root CA Request
	openssl req -new -nodes -text -out ${CERT_DIR}/root.csr \
	-keyout ${KEY_DIR}/root.key \
	-subj "/CN=localhost"

	printf "%s\n" "Signing root certificate request...."

	# Sign the Request to Make a Root CA certificate
	openssl x509 -req -in ${CERT_DIR}/root.csr -text -days 3650 \
		-signkey ${KEY_DIR}/root.key -out ${CERT_DIR}/root.crt

	chown -R ${UID}:${GID} ${CERT_DIR}/root.crt
fi

# svc-gis
if [ ! -f "${CERT_DIR}/client.${CLIENT}.crt" ]; then
	printf "%s\n" "Creating ${CLIENT} certificate request...."

	# Create Client CA Request
	openssl req -new -nodes -text -out ${CERT_DIR}/client.${CLIENT}.csr \
		-keyout ${KEY_DIR}/client.${CLIENT}.key -subj "/CN=postgis"
	# chmod og+rwx ${KEY_DIR}/client.${CLIENT}.key

	printf "%s\n" "Signing client request with root CA...."

	# Use the Root CA to Sign the Client Certificate
	openssl x509 -req -in ${CERT_DIR}/client.${CLIENT}.csr -text -days 3650 \
		-CA ${CERT_DIR}/root.crt \
		-CAkey ${KEY_DIR}/root.key -CAcreateserial \
		-out ${CERT_DIR}/client.${CLIENT}.crt

	# Create PKCS#8 format key
	openssl pkcs8 -topk8 -outform PEM -in ${KEY_DIR}/client.${CLIENT}.key -out ${KEY_DIR}/client.${CLIENT}.key.pk8 -nocrypt

	chown -R ${UID}:${GID} ${CERT_DIR}/client.${CLIENT}.crt
	chown -R ${UID}:${GID} ${KEY_DIR}/client.${CLIENT}.key.pk8
fi

exit 0
