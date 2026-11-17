# Password365 cryptography specification — format v1

Status: implemented in `core/password365-crypto` and `core/password365-vault`. All byte layouts below
are normative; any other implementation (Swift/Kotlin/TS) must match them bit for bit.

> **Format labels.** The app was called Quillock before 2026-11-13. The domain-separation labels
> (`quillock/v1/…`), the `.qlvault` store file and the `QLSYNC1` magic keep that name on purpose:
> they are part of format v1 and users never see them; changing them would make existing vaults
> and pairings unusable. What users see was renamed on 2026-11-16, reading the old forms still:
> the Secret Key prefix (`P365-`, formerly `Q1-`) and the file extensions (`.p365sync` and
> `.p365backup`, formerly `.qlsync` and `.qlbackup`).

## 1. Primitives

| Purpose | Algorithm | Parameters |
|---|---|---|
| Password hashing | Argon2id v1.3 | default desktop m=256 MiB, t=3, p=4 · mobile m=128 MiB, t=4, p=4 · **floor** m≥64 MiB, t≥3, 1≤p≤16 · output 32 B |
| Key derivation | HKDF-SHA-256 (RFC 5869) | output 32 B |
| Authenticated encryption | XChaCha20-Poly1305 | 256-bit key, 192-bit random nonce, 128-bit tag |
| Randomness | OS CSPRNG (`getrandom`) | rejection sampling for uniform integers |
| Password normalization | Unicode NFKD | no trimming |

## 2. Secret Key

- 16 random bytes.
- Text: `P365-` + Crockford base32 of the 16 bytes (26 symbols) + 2 check symbols, grouped by 4:
  `P365-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX`. The prefix is a label only (not in the check symbols,
  not in key derivation); `Q1-`, its spelling before 2026-11-16, is accepted when parsing.
- Check symbols = Crockford base32 of the first 10 bits of
  `SHA-256("quillock/v1/secret-key-check" ‖ key)`.
- Parsing is case-insensitive, ignores `-` and whitespace, maps `I`/`L`→`1` and `O`→`0`.

## 3. Account Unlock Key (AUK)

```
k_pw = Argon2id(NFKD(password), salt, params)                       # salt: 32 random bytes
k_sk = HKDF(ikm = secret_key, salt = store_id, info = "quillock/v1/secret-key")
AUK  = HKDF(ikm = k_pw ‖ k_sk, salt = salt,    info = "quillock/v1/auk")
```

`store_id` = 16 random bytes generated at store creation.

## 4. Associated data (AAD)

`Aad(domain, f1, f2, …) = len(domain) ‖ domain ‖ len(f1) ‖ f1 ‖ …` with every length a
4-byte big-endian `u32`; integers are 8-byte big-endian `u64` fields.

| Context | Domain | Fields |
|---|---|---|
| Keyset | `quillock/v1/keyset` | store_id, salt, memory_kib, iterations, parallelism |
| Keyset (store with an integrity sum) | `password365/v2/keyset` | store_id, salt, memory_kib, iterations, parallelism |
| Integrity sum | `password365/v1/integrity-sum` | — |
| Record | `quillock/v1/record` | store_id, record_id |
| Item head | `quillock/v1/item-head` | vault_id, item_id, version |
| Item details | `quillock/v1/item-details` | vault_id, item_id, version |

Binding the KDF parameters into the keyset AAD means a tampered (downgraded) header fails to decrypt;
in addition parameters below the floor are refused before hashing.

## 5. Sealed box

```
0x01 ‖ nonce (24 B) ‖ XChaCha20-Poly1305(key, nonce, plaintext, AAD) (ciphertext ‖ 16-B tag)
```

## 6. Envelope (records and item halves)

```
plaintext = postcard(value)
padded    = u32_be(len(plaintext)) ‖ plaintext ‖ 0x00…   # up to a multiple of 256 bytes
sealed    = SealedBox(key, padded, AAD)
```

## 7. Key hierarchy and storage

```
AUK ──wraps──▶ storage key (32 B random)           header.wrapped_keyset (AAD: keyset)
storage key ──seals──▶ record bodies                records.blob          (AAD: record)
   RecordBody::Vault { vault, key: vault key }
   RecordBody::Item  { item_id, vault_id, version, head, details }
vault key ──seals──▶ item head / item details      (AAD: item-head / item-details)
```

SQLite tables (`PRAGMA secure_delete = ON`, file mode 0600):

```
header(id=1, format_version, store_id, kdf_memory_kib, kdf_iterations, kdf_parallelism, salt, wrapped_keyset)
records(id BLOB PRIMARY KEY, blob BLOB)          -- id = vault/item UUID bytes
```

Records that fail to decrypt are ignored and **never deleted** (forward compatibility and future
hidden spaces, see `docs/ideas/secure-storage-and-unlock.md`).

### 7.1 Integrity sum

```
k_int = HKDF(ikm = storage key, salt = store_id, info = "password365/v1/record-integrity")
sum   = Σ HMAC-SHA-256(k_int, u32_be(len(id)) ‖ id ‖ blob)   mod 2^256, over every row of records
integrity(id=1, sum = SealedBox(k_int, sum, AAD: integrity sum))
```

Updated in the same write as each record and checked on unlock: records put back, removed or
added without the key are refused. The sum is sealed because it is linear: in the clear, two copies
of the file would tell how to undo exactly the changes between them.

A keyset wrapped with the `password365/v2/keyset` label **promises** the sum: such a store is
refused when the sum is missing or not sealed. A keyset with the original `quillock/v1/keyset`
label (written before the sum existed) still opens; on that unlock the store gets a sealed sum, a
**new storage key** (every record re-sealed) and a v2 keyset, so the old header cannot be put back
to skip the check. A v1 header that opens none of the store's records is refused. Quick-unlock
copies carry the same promise in their version byte (2, label `password365/v2/quick-unlock`).

## 8. Changing the master password

The storage key is re-wrapped under a new AUK with a **fresh salt**; records are untouched.

## 9. Test vectors

- HKDF: RFC 5869 test case 1 (`password365-crypto/src/hkdf.rs`).
- AEAD: fixed key/nonce regression vector (`password365-crypto/src/aead.rs`).
- **AUK end to end**: `core/password365-crypto/tests/vectors/auk.json` — password (one with a
  precomposed é, to exercise NFKD), Secret Key, account id, salt and Argon2id parameters (minimum and
  desktop) → `k_pw`, `k_sk`, `AUK`. The test `auk_vectors` fails if the derivation ever changes.
  Cross-checked with independent implementations: Argon2id via OpenSSL (Node `crypto.argon2Sync`),
  HKDF-SHA256 via Python's `hmac`. Ports (Swift, Kotlin) must reproduce every case.
