use crate::keys;

fn try_copy_to_clipboard(text: &str) -> bool {
    match arboard::Clipboard::new() {
        Ok(mut clipboard) => clipboard.set_text(text).is_ok(),
        Err(_) => false,
    }
}

pub fn run_whoami() -> anyhow::Result<()> {
    let keypair = keys::store::load_keypair()?;
    let public_key = keypair.public_key();
    let pubkey_uri = public_key.to_uri_string();
    let fingerprint = keys::fingerprint::short_fingerprint(&public_key);
    let homeserver = keys::store::read_homeserver()?;
    let key_path = keys::store::secret_key_path()?;

    println!("Public Key:  {}", pubkey_uri);
    println!("Fingerprint: {}", fingerprint);
    println!("Homeserver:  pk:{}", homeserver);
    println!("Key file:    {}", key_path.display());
    println!();

    if try_copy_to_clipboard(&pubkey_uri) {
        println!("Public key copied to clipboard.");
    } else {
        println!("(Clipboard unavailable — copy public key manually)");
    }

    Ok(())
}
