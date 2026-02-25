# cclink

Accountless, DHT-backed, end-to-end encrypted session handoff for [Claude Code](https://docs.anthropic.com/en/docs/claude-code). Your Ed25519 key is your identity; the [Mainline DHT](https://pkarr.org/) is your rendezvous. No SaaS, no relay, no logins.

```
Machine A                              Machine B
$ cclink                               $ cclink pickup
Session: abc12345 in ~/myproject       Resuming session abc12345...
Published!                             claude --resume abc12345
  Run on another machine:
  cclink pickup
```

## What it does

cclink grabs your current Claude Code session ID, encrypts it with your Ed25519/PKARR keypair, signs the record, and publishes it directly to the Mainline DHT as a PKARR SignedPacket. On another machine, `cclink pickup` resolves, verifies, decrypts, and launches `claude --resume` automatically.

No accounts. No central relay. No signup tokens. Your PKARR key is your identity.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/johnzilla/cclink/main/install.sh | sh
```

Or build from source:

```bash
cargo install --path .
```

## Quick start

```bash
# 1. Generate a keypair
cclink init

# 2. Start a Claude Code session, then publish a handoff
cclink

# 3. On another machine (with the same keypair), pick it up
cclink pickup
```

## Multi-machine setup

The default `cclink` / `cclink pickup` flow encrypts to **your keypair**. Pickup looks up the record using your public key, so both machines need the same key — or you need to use `--share` or `--pin` to bridge different keys.

### Option 1: Same key on both machines (simplest)

Copy your key from the first machine to the second:

```bash
# On machine A (where the key already exists)
cat ~/.pubky/secret_key | ssh machineB 'mkdir -p ~/.pubky && cat > ~/.pubky/secret_key && chmod 600 ~/.pubky/secret_key'

# Or use init --import on machine B
scp machineA:~/.pubky/secret_key /tmp/key
cclink init --import /tmp/key
rm /tmp/key
```

Now `cclink` and `cclink pickup` work seamlessly across both machines.

### Option 2: `--share` (different keys, encrypt for the other machine)

Each machine keeps its own key. You encrypt specifically for the recipient's public key:

```bash
# On machine B (the new machine), get its public key
cclink whoami    # → pk:yro4u8t5...

# On machine A, publish targeting machine B's key
cclink --share yro4u8t5...

# On machine B, pick up from machine A's key
cclink whoami    # run on machine A first to get its pubkey
cclink pickup <machineA-pubkey>
```

### Option 3: `--pin` (different keys, no key exchange needed)

Protect the handoff with a PIN instead of a keypair. Anyone with the PIN and the publisher's public key can decrypt it:

```bash
# On machine A
cclink --pin                        # prompted to set a PIN

# On machine B
cclink pickup <machineA-pubkey>     # prompted to enter the PIN
```

Get machine A's public key with `cclink whoami` on machine A.

## Commands

### Publish (default)

Publishes the current Claude Code session as an encrypted handoff record.

```bash
cclink                          # auto-discover current session
cclink <session-id>             # publish a specific session ID
cclink --ttl 3600               # expire in 1 hour (default: 86400 = 24h)
cclink --burn                   # revoke after first pickup
cclink --pin                    # protect with a PIN (prompted)
cclink --share <pubkey>         # encrypt for a specific recipient
cclink --qr                     # show QR code after publish
```

### Pickup

Retrieves and resumes a handoff.

```bash
cclink pickup                   # pick up your own latest handoff
cclink pickup <pubkey>          # pick up from another user (--share)
cclink pickup -y                # skip confirmation prompt
cclink pickup --qr              # show session ID as QR code
```

### Init

Generate or import a PKARR keypair. By default, the key is encrypted with a passphrase (min 8 characters).

```bash
cclink init                             # generate a passphrase-protected keypair
cclink init --no-passphrase             # generate an unprotected plaintext keypair
cclink init --import /path/to/key       # import from file (encrypted by default)
echo <hex> | cclink init --import -     # import from stdin
```

### Whoami

Show your identity.

```bash
cclink whoami
# Public Key:  pk:abc123...
# Fingerprint: AB:CD:EF:12
# Key file:    /home/user/.pubky/secret_key
```

### List

Show the active handoff record on the DHT.

```bash
cclink list
```

### Revoke

Revoke the active handoff record from the DHT.

```bash
cclink revoke                   # revoke with confirmation
cclink revoke -y                # skip confirmation
```

## Encryption modes

| Mode | Flag | Who can decrypt |
|------|------|-----------------|
| Self (default) | _(none)_ | Only you (your X25519 key derived from Ed25519) |
| Shared | `--share <pubkey>` | Only the specified recipient |
| PIN | `--pin` | Anyone with the PIN (minimum 8 characters) |
| Burn | `--burn` | Revoked after first successful pickup |

Modes can be combined: `cclink --burn --pin` creates a PIN-protected, single-use handoff.

## Architecture

```
                   publish (signed + encrypted)
  ┌──────────┐    ─────────────────────────────►    ┌───────────────┐
  │  cclink   │       PKARR SignedPacket            │   Mainline    │
  │  (Rust)   │                                     │     DHT       │
  │           │    ◄─────────────────────────────    │               │
  └──────────┘       pickup (verify + decrypt)      └───────────────┘
```

- **Identity**: Ed25519 keypair via [PKARR](https://pkarr.org/) — the same key format used across the Pubky ecosystem
- **Transport**: [PKARR Mainline DHT](https://crates.io/crates/pkarr) — records are published as DNS TXT records inside Ed25519-signed packets, addressed by public key
- **Encryption**: [age](https://age-encryption.org/) (X25519) for the full payload (session ID + hostname + project path); Ed25519 keys are converted to X25519 for encryption. No metadata is visible in cleartext on the DHT.
- **Signing**: Dual signatures — PKARR packet signature (DHT authentication) + inner Ed25519 signature over canonical JSON (defense in depth)

## Security model

| Threat | Mitigation |
|--------|------------|
| DHT node reads session IDs | Session IDs are age-encrypted inside the payload blob; DHT nodes see only ciphertext |
| DHT node reads hostname/project | Hostname and project path are encrypted inside the payload blob alongside the session ID — no metadata leakage |
| Forged handoff record | Dual Ed25519 signature verification (PKARR packet + inner record) |
| Replay attack | TTL expiry + optional burn-after-read |
| Intercepted QR/link | PIN mode adds a second factor; burn mode limits the window |
| Key compromise | Keys encrypted at rest with passphrase (Argon2id + age); 0600 permissions; secret material zeroized from memory after use |

**Key principle**: No session content or metadata transits the network in cleartext. The entire payload (session ID, hostname, project path) is encrypted into a single blob. The outer record contains only the ciphertext, timestamps, public key, and flags. The pickup device still needs access to `~/.claude/sessions/` (via shared filesystem, SSH, Tailscale, etc.) to actually resume the session.

**At rest**: Keys are stored in a CCLINKEK encrypted envelope by default. Existing plaintext key files from v1.2 and earlier continue to work without any passphrase prompt.

**Key derivation**: Both PINs and key passphrases use Argon2id (64 MB memory, 3 iterations, 1 parallelism) followed by HKDF-SHA256 with domain-separated info strings. This is the same memory-hard KDF recommended by OWASP and used by 1Password, Bitwarden, and Signal. Brute-forcing a passphrase requires ~64 MB per guess — GPU/ASIC attacks don't scale.

## Why not just use `/remote-control`?

Claude Code's [Remote Control](https://docs.anthropic.com/en/docs/claude-code/remote) (`/remote-control`) is tied to Anthropic accounts and infrastructure, optimized for terminal-to-mobile control, and opaque from a security and infrastructure perspective.

cclink is local-first, self-host-friendly, and composable with your existing SSH/tmux/Tailscale story. You own the keys, you own the transport, you see exactly what crosses the wire.

**Compatible with `/remote-control`**: use both. cclink handles *which box* runs the session; `/remote-control` handles *which UI* you control it from.

## Troubleshooting

### "Record not found" on pickup

This almost always means **your two machines have different keypairs**. When you ran `cclink init` on each machine, each got a unique key. Pickup looks for a record under *its own* public key — if the record was published under a different key, it won't find it.

**Fix**: See [Multi-machine setup](#multi-machine-setup) above. Either copy the same key to both machines, or use `--share`/`--pin` to bridge different keys.

**Other causes**: the record expired (default TTL is 24 hours), was revoked (`--burn` after first pickup), or DHT propagation hasn't completed yet (retry in a few seconds).

### "This handoff was shared with ..."

You published with `--share <pubkey>` — only the specified recipient can decrypt it. Run `cclink pickup <publisher-pubkey>` on the recipient machine.

### "Incorrect PIN"

The PIN you entered doesn't match the one used during publish. PINs are case-sensitive and must be at least 8 characters.

## License

MIT
