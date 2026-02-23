# cclink

Secure session handoff for [Claude Code](https://docs.anthropic.com/en/docs/claude-code) between devices, powered by [Pubky](https://pubky.org/) decentralized identity.

```
Machine A                              Machine B
$ cclink                               $ cclink pickup
Session: abc12345 in ~/myproject       Resuming session abc12345...
Published!                             claude --resume abc12345
  Run on another machine:
  cclink pickup
```

## What it does

cclink grabs your current Claude Code session ID, encrypts it with your Ed25519/PKARR keypair, signs the record, and publishes it to a Pubky homeserver. On another machine, `cclink pickup` retrieves, verifies, decrypts, and launches `claude --resume` automatically.

No accounts. No central relay. Your PKARR key is your identity.

## Install

```bash
cargo install --path .
```

Requires Rust 1.70+.

## Quick start

```bash
# 1. Generate a keypair (stored in ~/.pubky/secret_key)
cclink init

# 2. Start a Claude Code session, then publish a handoff
cclink

# 3. On another machine (with the same keypair), pick it up
cclink pickup
```

## Commands

### Publish (default)

Publishes the current Claude Code session as an encrypted handoff record.

```bash
cclink                          # auto-discover current session
cclink <session-id>             # publish a specific session ID
cclink --ttl 3600               # expire in 1 hour (default: 86400 = 24h)
cclink --burn                   # delete after first pickup
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

Generate or import a PKARR keypair.

```bash
cclink init                             # generate new keypair
cclink init --import /path/to/key       # import from file
echo <hex> | cclink init --import -     # import from stdin
cclink init --homeserver <pubkey>       # use a custom homeserver
```

### Whoami

Show your identity.

```bash
cclink whoami
# Public Key:  pk:abc123...
# Fingerprint: AB:CD:EF:12
# Homeserver:  pk:8um71us3fyw6h8wbcxb5ar3rwusy1a6u49956ikzojg3gcwd1dty
# Key file:    /home/user/.pubky/secret_key
```

### List

Show all active handoff records on your homeserver.

```bash
cclink list
```

### Revoke

Delete handoff records from the homeserver.

```bash
cclink revoke <token>           # delete a specific handoff
cclink revoke --all             # delete all handoffs
cclink revoke --all -y          # skip confirmation
```

## Encryption modes

| Mode | Flag | Who can decrypt |
|------|------|-----------------|
| Self (default) | _(none)_ | Only you (your X25519 key derived from Ed25519) |
| Shared | `--share <pubkey>` | Only the specified recipient |
| PIN | `--pin` | Anyone with the PIN |
| Burn | `--burn` | Deleted after first successful pickup |

Modes can be combined: `cclink --burn --pin` creates a PIN-protected, single-use handoff.

## Architecture

```
                   publish (signed + encrypted)
  ┌──────────┐    ─────────────────────────────►    ┌───────────────┐
  │  cclink   │        Pubky SDK (PKARR)            │    Pubky      │
  │  (Rust)   │                                     │  Homeserver   │
  │           │    ◄─────────────────────────────    │               │
  └──────────┘       pickup (verify + decrypt)      └───────────────┘
```

- **Identity**: Ed25519 keypair via [PKARR](https://pkarr.org/) — the same key format used across the Pubky ecosystem
- **Transport**: [Pubky SDK](https://crates.io/crates/pubky) handles homeserver discovery via PKARR, authentication, and all CRUD operations
- **Encryption**: [age](https://age-encryption.org/) (X25519) for session payloads; Ed25519 keys are converted to X25519 for encryption
- **Signing**: Every record is Ed25519-signed over canonical JSON; signature is verified on pickup

## Security model

| Threat | Mitigation |
|--------|------------|
| Homeserver reads session IDs | Session IDs are age-encrypted; homeserver sees only ciphertext |
| Forged handoff record | Ed25519 signature verification on all records |
| Replay attack | TTL expiry + optional burn-after-read |
| Intercepted QR/link | PIN mode adds a second factor; burn mode limits the window |
| Key compromise | Keys stored with 0600 permissions; cclink refuses to read keys with looser perms |

**Key principle**: No session content transits the network. Only the encrypted session ID and metadata are published. The pickup device still needs access to `~/.claude/sessions/` (via shared filesystem, SSH, Tailscale, etc.) to actually resume the session.

## License

MIT
