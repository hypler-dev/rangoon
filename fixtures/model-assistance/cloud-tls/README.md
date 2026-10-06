# Public synthetic TLS fixtures

These certificates and private keys are intentionally public test material. They
must never authenticate a production server or be added to a production trust
store. The cloud adapter's private `cfg(test)` dialer uses them only with disposable
loopback servers. The production client always uses bundled WebPKI public roots.

`valid.der` names `api.openai.com`; `wrong.der` names `wrong.example`. Both are
signed by the disposable test CA `ca.der`. `*-key.der` are unencrypted PKCS#8 test
keys. The CA private key is not included. Generated October 6, 2026 with OpenSSL,
RSA-2048/SHA-256, serverAuth, CA:FALSE, and a 3,650-day validity. Regenerate before
expiry; do not disable time/name/signature validation to keep tests passing.
