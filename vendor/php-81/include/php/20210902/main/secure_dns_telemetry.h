/*
 * secure_dns_telemetry.h
 * Client Library for Secure DNS Telemetry (Corrected)
 * Features:
 * - Ciphertext Splitting: Splits >63 char payloads into multiple DNS labels
 * - Strict Memory Safety: No buffer overruns or unaligned access
 * - Direct UDP Connection: Validates Source IP/Port
 */

#pragma once

#include <stdint.h>
#ifndef _GNU_SOURCE
#define _GNU_SOURCE 1
#endif

#ifdef __cplusplus
extern "C" {
#endif

#include <arpa/inet.h>
#include <ctype.h>
#include <errno.h>
#include <netdb.h>
#include <netinet/in.h>
#include <sodium.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/time.h>
#include <time.h>
#include <unistd.h>

/* --- CONFIGURATION --- */

/*
 * RUNTIME CONFIGURATION:
 * The telemetry system accepts host and key as runtime parameters.
 * No build-time configuration is required.
 *
 * Usage:
 *   telemetry_check(host, port, server_pk_b64, package, version)
 *
 * Generate server key:
 *   ./secure_dns_telemetry_gen_key server.key
 */

#define EDNS_PAYLOAD_SIZE  1232
#define DNS_LABEL_SIZE	   63
#define FIXED_PAYLOAD_SIZE 96

/* Logging - override this for integration (e.g., php_error_docref) */
#ifndef TELEMETRY_LOG
#define TELEMETRY_LOG(...) fprintf(stderr, "[php-telemetry] " __VA_ARGS__)
#endif

/* Default DNS port */
#ifndef TELEMETRY_DNS_PORT
#define TELEMETRY_DNS_PORT "53"
#endif

typedef struct {
	unsigned char pk[crypto_box_PUBLICKEYBYTES];
	unsigned char sk[crypto_box_SECRETKEYBYTES];
	unsigned char nonce[crypto_box_NONCEBYTES];
} session_ctx_t;

/* --- HELPERS --- */

static inline uint16_t
read_u16(uint8_t **ptrp) {
	uint16_t val;
	memcpy(&val, *ptrp, sizeof(val));
	*ptrp += sizeof(val);
	return ntohs(val);
}

static inline uint32_t
read_u32(uint8_t **ptrp) {
	uint32_t val;
	memcpy(&val, *ptrp, sizeof(val));
	*ptrp += sizeof(val);
	return ntohl(val);
}

static inline void
write_u16(uint8_t **ptrp, uint16_t val) {
	uint16_t wire = htons(val);
	memcpy(*ptrp, &wire, sizeof(wire));
	*ptrp += sizeof(wire);
}

static inline void
write_u32(uint8_t **ptrp, uint32_t val) {
	uint32_t wire = htonl(val);
	memcpy(*ptrp, &wire, sizeof(wire));
	*ptrp += sizeof(wire);
}

static inline int
validate_package_name(const char *pkg) {
	if (!pkg || strlen(pkg) == 0 || strlen(pkg) > 63) {
		return -1;
	}
	for (const char *p = pkg; *p; p++) {
		/* Only allow alphanumeric, dash, dot, underscore */
		if (!isalnum((unsigned char)*p) && *p != '-' && *p != '.' &&
		    *p != '_') {
			return -1;
		}
	}
	return 0;
}

static inline void
sanitize_version(char *dest, const char *src, size_t dest_size) {
	const char *start = src;
	const char *colon = strchr(src, ':');
	if (colon) {
		start = colon + 1;
	}

	size_t i = 0;
	while (*start != '\0' && *start != '+' && *start != '~' &&
	       i < dest_size - 1) {
		dest[i++] = *start++;
	}
	dest[i] = '\0';
}

static inline int
append_dns_label(uint8_t **ptr, const uint8_t *end, const char *label,
		 size_t len) {
	if (len > 63) {
		return -1;
	}
	if (*ptr + len + 1 >= end) {
		return -1;
	}
	*(*ptr)++ = (uint8_t)len;
	if (len > 0 && label != NULL) {
		memcpy(*ptr, label, len);
	}
	*ptr += len;
	return 0;
}

static inline int
encode_dns_label(uint8_t **ptrp, const uint8_t *end, const uint8_t *src,
		 size_t src_len) {
	char b64[DNS_LABEL_SIZE];
	size_t b64_len = sizeof(b64);
	size_t max_len = sodium_base64_ENCODED_LEN(
		src_len, sodium_base64_VARIANT_URLSAFE_NO_PADDING);
	if (max_len > b64_len) {
		return -1;
	}
	sodium_bin2base64(b64, b64_len, src, src_len,
			  sodium_base64_VARIANT_URLSAFE_NO_PADDING);
	return append_dns_label(ptrp, end, b64, strlen(b64));
}

static inline int
append_dns_suffix(uint8_t **ptrp, const uint8_t *end, const char *suffix) {
	char suffix_copy[256];
	memset(suffix_copy, 0, sizeof(suffix_copy));
	if (memccpy(suffix_copy, suffix, '\0', sizeof(suffix_copy)) == NULL) {
		return -1;
	}

	/* H5 FIX: Use strtok_r for thread safety */
	char *saveptr;
	char *token = strtok_r(suffix_copy, ".", &saveptr);
	while (token) {
		size_t len = strlen(token);
		if (append_dns_label(ptrp, end, token, len) != 0) {
			return -1;
		}
		if (len == 0) {
			/* Root Label was part of the suffix */
			return 0;
		}
		token = strtok_r(NULL, ".", &saveptr);
	}
	/* Append Root Label if not part of the suffix */
	append_dns_label(ptrp, end, NULL, 0);

	return 0;
}

static inline uint8_t *
skip_dns_name(uint8_t *ptr, uint8_t *end) {
	while (ptr < end) {
		if (*ptr == 0) {
			/* Root Label */
			ptr += 1;
			break;
		} else if ((*ptr & 0xC0) == 0xC0) {
			/* C2 FIX: Reject compressed labels to prevent pointer
			 * attacks */
			return NULL;
		}

		/* Regular Label */
		uint8_t label_len = *ptr;
		if (ptr + label_len + 1 > end) {
			return NULL;
		}
		ptr += (label_len + 1);
	}
	if (ptr > end) {
		return NULL;
	}
	return ptr;
}

static inline int
validate_peer(const struct sockaddr *target, const struct sockaddr *source) {
	if (target->sa_family != source->sa_family) {
		return 0;
	}
	if (target->sa_family == AF_INET) {
		struct sockaddr_in *t4 = (struct sockaddr_in *)target;
		struct sockaddr_in *s4 = (struct sockaddr_in *)source;
		return t4->sin_port == s4->sin_port &&
		       memcmp(&t4->sin_addr, &s4->sin_addr,
			      sizeof(t4->sin_addr)) == 0;
	} else if (target->sa_family == AF_INET6) {
		struct sockaddr_in6 *t6 = (struct sockaddr_in6 *)target;
		struct sockaddr_in6 *s6 = (struct sockaddr_in6 *)source;
		return t6->sin6_port == s6->sin6_port &&
		       memcmp(&t6->sin6_addr, &s6->sin6_addr,
			      sizeof(t6->sin6_addr)) == 0;
	}
	return 0;
}

static inline int
build_edns_packet(unsigned char *buf, size_t buf_len, uint16_t tx_id,
		  const char *pkg, const char *version, session_ctx_t *ctx,
		  const unsigned char *server_pk, const char *domain_suffix) {
	unsigned char *ptr = buf;
	unsigned char *end = buf + buf_len;

	if (12 > buf_len) {
		return -1;
	}
	write_u16(&ptr, tx_id);	 /* ID */
	write_u16(&ptr, 0x0000); /* Flags */
	write_u16(&ptr, 1);	 /* QDCOUNT=1 */
	write_u16(&ptr, 0);	 /* ANCOUNT=0 */
	write_u16(&ptr, 0);	 /* NSCOUNT=0 */
	write_u16(&ptr, 1);	 /* ARCOUNT=1 */

	/* Crypto */
	crypto_box_keypair(ctx->pk, ctx->sk);
	randombytes_buf(ctx->nonce, sizeof(ctx->nonce));

	/* M3 FIX: Build versioned payload: v1|pkg|version|timestamp */
	time_t now = time(NULL);
	uint8_t padded_payload[FIXED_PAYLOAD_SIZE];
	memset(padded_payload, 0, FIXED_PAYLOAD_SIZE);
	int payload_len = snprintf((char *)padded_payload, FIXED_PAYLOAD_SIZE,
				   "v1|%s|%s|%ld", pkg, version, (long)now);
	if (payload_len < 0 || payload_len >= FIXED_PAYLOAD_SIZE) {
		return -1;
	}

	/* Single encryption */
	uint8_t ciphertext[FIXED_PAYLOAD_SIZE + crypto_box_MACBYTES];
	if (crypto_box_easy(ciphertext, padded_payload, FIXED_PAYLOAD_SIZE,
			    ctx->nonce, server_pk, ctx->sk) != 0) {
		return -1;
	}

	/* Base64 encode ciphertext */
	size_t cipher_len = sizeof(ciphertext);
	size_t b64_max_len = sodium_base64_ENCODED_LEN(
		cipher_len, sodium_base64_VARIANT_URLSAFE_NO_PADDING);
	char b64_cipher[256];
	if (b64_max_len > sizeof(b64_cipher)) {
		return -1;
	}
	sodium_bin2base64(b64_cipher, sizeof(b64_cipher), ciphertext,
			  cipher_len, sodium_base64_VARIANT_URLSAFE_NO_PADDING);

	/* Encode public key and nonce labels */
	if (encode_dns_label(&ptr, end, ctx->pk, crypto_box_PUBLICKEYBYTES) !=
	    0) {
		return -1;
	}
	if (encode_dns_label(&ptr, end, ctx->nonce, crypto_box_NONCEBYTES) !=
	    0) {
		return -1;
	}

	/* Split base64 ciphertext into DNS labels (max 63 chars each) */
	size_t b64_len = strlen(b64_cipher);
	size_t offset = 0;
	while (offset < b64_len) {
		size_t chunk_len = b64_len - offset;
		if (chunk_len > DNS_LABEL_SIZE) {
			chunk_len = DNS_LABEL_SIZE;
		}
		if (append_dns_label(&ptr, end, b64_cipher + offset,
				     chunk_len) != 0) {
			return -1;
		}
		offset += chunk_len;
	}

	if (append_dns_suffix(&ptr, end, domain_suffix) != 0) {
		return -1;
	}

	if (ptr + 4 > end) {
		return -1;
	}
	write_u16(&ptr, 16); /* TXT QTYPE */
	write_u16(&ptr, 1);  /* IN QCLASS */

	/* EDNS0 OPT */
	if (ptr + 11 > end) {
		return -1;
	}
	*ptr++ = 0;			    /* OWNER */
	write_u16(&ptr, 41);		    /* TYPE */
	write_u16(&ptr, EDNS_PAYLOAD_SIZE); /* CLASS */
	write_u32(&ptr, 0);		    /* TTL */
	write_u16(&ptr, 0);		    /* RDLEN */

	return (int)(ptr - buf);
}

static inline int
decrypt_payload(uint8_t *ptr, uint16_t rdlen, session_ctx_t *ctx,
		const unsigned char *server_pk) {
	unsigned char *rdata_ptr = ptr;
	unsigned char *rdata_end = ptr + rdlen;

	while (rdata_ptr < rdata_end) {
		int txt_len = *rdata_ptr;
		rdata_ptr++;
		if (rdata_ptr + txt_len > rdata_end) {
			break;
		}
		if (txt_len == 0) {
			continue;
		}

		char b64_resp[512];
		if (txt_len > 511) {
			txt_len = 511;
		}
		memcpy(b64_resp, rdata_ptr, txt_len);
		b64_resp[txt_len] = '\0';

		size_t bin_len = 0;
		unsigned char bin[512];

		if (sodium_base642bin(
			    bin, sizeof(bin), b64_resp, txt_len, NULL, &bin_len,
			    NULL,
			    sodium_base64_VARIANT_URLSAFE_NO_PADDING) != 0) {
			return -1;
		}

		if (bin_len <= crypto_box_NONCEBYTES + crypto_box_MACBYTES) {
			return -1;
		}

		unsigned char *nonce = bin;
		unsigned char *ciphertext = bin + crypto_box_NONCEBYTES;
		size_t cipher_len = bin_len - crypto_box_NONCEBYTES;

		/* H3 FIX: Validate plaintext size before decryption */
		size_t plaintext_len = cipher_len - crypto_box_MACBYTES;
		if (plaintext_len >= 256) {
			return -1;
		}

		unsigned char decrypted[256];
		if (crypto_box_open_easy(decrypted, ciphertext, cipher_len,
					 nonce, server_pk, ctx->sk) != 0) {
			return -1;
		}

		decrypted[plaintext_len] = '\0';
		if (strstr((char *)decrypted, "\"urgency\":\"high\"") ||
		    strstr((char *)decrypted, "\"urgency\":\"critical\"") ||
		    strstr((char *)decrypted, "\"urgency\":\"emergency\"")) {
			TELEMETRY_LOG("Security Alert: %s\n",
				      (char *)decrypted);
		}

		rdata_ptr += txt_len;
	}

	return 0;
}

static inline void
handle_response(unsigned char *buf, int len, session_ctx_t *ctx, uint16_t tx_id,
		const unsigned char *server_pk) {
	if (len < 12) {
		return;
	}

	unsigned char *end = buf + len;
	unsigned char *ptr = buf;
	uint16_t resp_id = read_u16(&ptr);
	if (resp_id != tx_id) {
		return;
	}

	/* M5 FIX: Validate DNS response code */
	uint16_t flags = read_u16(&ptr);
	uint16_t rcode = flags & 0x000F;
	if (rcode != 0) {
		/* RCODE != NOERROR, reject response */
		return;
	}

	uint16_t qdcount = read_u16(&ptr);
	if (qdcount != 1) {
		return;
	}

	ptr = skip_dns_name(ptr, end);
	if (ptr == NULL || ptr + 4 > end) {
		return;
	}
	uint16_t qtype = read_u16(&ptr);
	if (qtype != 16) {
		return;
	}
	uint16_t qclass = read_u16(&ptr);
	if (qclass != 1) {
		return;
	}

	uint16_t ancount = read_u16(&ptr);
	for (size_t i = 0; i < ancount; i++) {
		if (ptr >= end) {
			return;
		}
		ptr = skip_dns_name(ptr, end);
		if (ptr == NULL || ptr + 10 > end) {
			return;
		}

		uint16_t atype = read_u16(&ptr);
		uint16_t aclass = read_u16(&ptr);
		uint32_t attl = read_u32(&ptr);
		uint16_t rdlen = read_u16(&ptr);

		if (ptr + rdlen > end) {
			return;
		}

		(void)attl;

		switch (aclass) {
		case 1:
			switch (atype) {
			case 16:
				decrypt_payload(ptr, rdlen, ctx, server_pk);
				break;
			default:
				break;
			}
		default:
			break;
		}
		ptr += rdlen;
	}
}

static inline void
telemetry_check(const char *host, const char *port, const char *server_pk_b64,
		const char *package_name, const char *raw_version) {
	if (sodium_init() == -1) {
		return;
	}

	/* Decode Base64 public key */
	unsigned char server_pk[crypto_box_PUBLICKEYBYTES];
	size_t decoded_len;
	if (sodium_base642bin(server_pk, sizeof(server_pk), server_pk_b64,
			      strlen(server_pk_b64), NULL, &decoded_len, NULL,
			      sodium_base64_VARIANT_URLSAFE_NO_PADDING) != 0 ||
	    decoded_len != crypto_box_PUBLICKEYBYTES) {
		/* Invalid key format, disable telemetry */
		return;
	}

	/* C1 FIX: Validate package name to prevent injection */
	if (validate_package_name(package_name) != 0) {
		return;
	}

	char clean_ver[64];
	sanitize_version(clean_ver, raw_version, sizeof(clean_ver));

	session_ctx_t ctx;
	unsigned char buffer[EDNS_PAYLOAD_SIZE];
	uint16_t tx_id;
	randombytes_buf(&tx_id, sizeof(tx_id));

	/* Build domain suffix from host */
	char domain_suffix[256];
	snprintf(domain_suffix, sizeof(domain_suffix), "%s.", host);

	int packet_len = build_edns_packet(buffer, sizeof(buffer), tx_id,
					   package_name, clean_ver, &ctx,
					   server_pk, domain_suffix);
	if (packet_len <= 0) {
		goto cleanup;
	}

	struct addrinfo hints, *res, *p;
	memset(&hints, 0, sizeof(hints));
	hints.ai_family = AF_UNSPEC;
	hints.ai_socktype = SOCK_DGRAM;
	if (getaddrinfo(host, port ? port : TELEMETRY_DNS_PORT, &hints, &res) !=
	    0) {
		goto cleanup;
	}

	int sock = -1;
	struct sockaddr_storage target_addr;
	socklen_t target_len = 0;

	for (size_t pass = 0; pass < 2; pass++) {
		for (p = res; p != NULL; p = p->ai_next) {
			int match = (pass == 0) ? (p->ai_family == AF_INET6)
						: (p->ai_family == AF_INET);
			if (match) {
				sock = socket(p->ai_family, p->ai_socktype,
					      p->ai_protocol);
				if (sock >= 0) {
					memcpy(&target_addr, p->ai_addr,
					       p->ai_addrlen);
					target_len = p->ai_addrlen;
					goto connected;
				}
			}
		}
	}
connected:
	freeaddrinfo(res);
	if (sock >= 0) {
		/* M8 FIX: Check setsockopt() return value */
		struct timeval tv = { 2, 0 };
		if (setsockopt(sock, SOL_SOCKET, SO_RCVTIMEO, (const char *)&tv,
			       sizeof tv) != 0) {
			close(sock);
			goto cleanup;
		}

		/* Connect UDP socket for proper response routing */
		if (connect(sock, (struct sockaddr *)&target_addr,
			    target_len) < 0) {
			close(sock);
			goto cleanup;
		}

		ssize_t sent = send(sock, (const char *)buffer, packet_len, 0);
		if (sent >= 0) {
			int n = recv(sock, (char *)buffer, sizeof(buffer), 0);
			if (n > 0) {
				handle_response(buffer, n, &ctx, tx_id,
						server_pk);
			}
		}
		close(sock);
	}

cleanup:
	sodium_memzero(&ctx, sizeof(ctx));
	sodium_memzero(buffer, sizeof(buffer));
	sodium_memzero(clean_ver, sizeof(clean_ver));
	sodium_memzero(server_pk, sizeof(server_pk));
}

#ifdef __cplusplus
}
#endif
