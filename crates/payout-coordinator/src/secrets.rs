//! Authenticated, context-bound encryption for private database fields and member files.
use anyhow::{Context, Result, bail};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use rand_core::{OsRng, RngCore};
use std::{
    fs,
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
};
use zeroize::Zeroizing;

pub struct Secrets(Zeroizing<[u8; 32]>);
pub fn read_private(path: &Path) -> Result<Zeroizing<Vec<u8>>> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.permissions().mode() & 0o077 != 0 {
        bail!("private file must be regular and mode 0600 or stricter");
    }
    Ok(Zeroizing::new(fs::read(path)?))
}
impl Secrets {
    pub fn load(path: &Path) -> Result<Self> {
        let meta = fs::symlink_metadata(path)?;
        if !meta.is_file() || meta.permissions().mode() & 0o077 != 0 {
            bail!("secret key must be a regular file with mode 0600 or stricter");
        }
        let raw = Zeroizing::new(fs::read(path)?);
        Ok(Self(Zeroizing::new(
            raw.as_slice()
                .try_into()
                .context("secret key must be 32 raw bytes")?,
        )))
    }
    pub fn seal(&self, context: &str, plaintext: &[u8]) -> Result<Vec<u8>> {
        let mut nonce = [0; 24];
        OsRng.fill_bytes(&mut nonce);
        let mut blob = nonce.to_vec();
        blob.extend(
            XChaCha20Poly1305::new((&*self.0).into())
                .encrypt(
                    XNonce::from_slice(&nonce),
                    Payload {
                        msg: plaintext,
                        aad: context.as_bytes(),
                    },
                )
                .map_err(|_| anyhow::anyhow!("encryption failed"))?,
        );
        Ok(blob)
    }
    pub fn open(&self, context: &str, blob: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
        if blob.len() < 40 {
            bail!("invalid encrypted secret");
        }
        let plaintext = XChaCha20Poly1305::new((&*self.0).into())
            .decrypt(
                XNonce::from_slice(&blob[..24]),
                Payload {
                    msg: &blob[24..],
                    aad: context.as_bytes(),
                },
            )
            .map_err(|_| anyhow::anyhow!("secret authentication failed"))?;
        Ok(Zeroizing::new(plaintext))
    }
}

pub fn write_private_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wrong_key_context_and_modified_blob_fail() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        write_private_new(&a, &[1; 32]).unwrap();
        write_private_new(&b, &[2; 32]).unwrap();
        let key = Secrets::load(&a).unwrap();
        let other = Secrets::load(&b).unwrap();
        let mut encrypted = key.seal("market/nk", b"private-key-material").unwrap();
        assert!(!encrypted.windows(20).any(|s| s == b"private-key-material"));
        assert_eq!(
            &*key.open("market/nk", &encrypted).unwrap(),
            b"private-key-material"
        );
        assert!(other.open("market/nk", &encrypted).is_err());
        assert!(key.open("other/nk", &encrypted).is_err());
        encrypted[25] ^= 1;
        assert!(key.open("market/nk", &encrypted).is_err());
    }
}
