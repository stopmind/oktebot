use anyhow::{Result, anyhow, bail};
use base64::{Engine, prelude::BASE64_URL_SAFE_NO_PAD};
use rand::{RngExt, SeedableRng, rngs::SysRng};
use rand_chacha::ChaCha20Rng;
use std::{
    collections::BTreeMap,
    ops::DerefMut,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use teloxide::types::UserId;
use tokio::time::sleep;

const TOKEN_LIVE_DURATION: Duration = Duration::from_hours(24);

type TokenBase = [u8; 9];

struct LoginInner {
    rng: ChaCha20Rng,
    tokens_users: BTreeMap<TokenBase, (UserId, Instant)>,
    users_tokens: BTreeMap<UserId, TokenBase>,
}

pub struct Login(Mutex<LoginInner>);

impl Default for Login {
    fn default() -> Self {
        Login(
            LoginInner {
                tokens_users: BTreeMap::new(),
                users_tokens: BTreeMap::new(),
                rng: ChaCha20Rng::try_from_rng(&mut SysRng).unwrap(),
            }
            .into(),
        )
    }
}

impl Login {
    pub fn create_token(&self, id: UserId) -> Result<String> {
        let mut inner = self.0.lock().unwrap();

        let token_val = if let Some(token_val) = inner.users_tokens.get(&id) {
            let token_val = *token_val;
            if let Some((_, expiration_time)) = inner.tokens_users.get_mut(&token_val) {
                *expiration_time = Instant::now() + TOKEN_LIVE_DURATION;
            }
            token_val
        } else {
            let mut token_val: TokenBase = [0; _];
            let mut remain_attempts = 20;

            loop {
                inner.rng.fill(&mut token_val);
                if !inner.tokens_users.contains_key(&token_val) {
                    break;
                }
                remain_attempts -= 1;
                if remain_attempts == 0 {
                    bail!("all attempts failed")
                }
            }

            inner
                .tokens_users
                .insert(token_val, (id, Instant::now() + TOKEN_LIVE_DURATION));
            inner.users_tokens.insert(id, token_val);
            token_val
        };

        Ok(BASE64_URL_SAFE_NO_PAD.encode(token_val))
    }

    pub fn check_token(&self, token: &str) -> Result<UserId> {
        let token_val = BASE64_URL_SAFE_NO_PAD.decode(token)?;
        let chunk = token_val
            .first_chunk()
            .ok_or_else(|| anyhow!("invalid token"))?;

        let this = self.0.lock().unwrap();
        let (id, instant) = this
            .tokens_users
            .get(chunk)
            .ok_or_else(|| anyhow!("unknown token"))?;

        if instant > &Instant::now() {
            Ok(*id)
        } else {
            bail!("token expired")
        }
    }

    fn clear_tokens(&self) {
        let mut this = self.0.lock().unwrap();
        let this = this.deref_mut();
        let now = Instant::now();
        this.tokens_users.retain(|_, (user, expiration_time)| {
            let is_ok = *expiration_time < now;

            if !is_ok {
                this.users_tokens.remove(user);
            }

            is_ok
        })
    }

    pub async fn clear_token_worker(self: Arc<Self>) {
        loop {
            sleep(TOKEN_LIVE_DURATION).await;
            self.clear_tokens();
        }
    }
}
