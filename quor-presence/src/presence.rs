use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle, sleep};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use discord_rich_presence::activity::{Activity, Assets, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};

use crate::config::Profile;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Status {
    // дискорд не отвечает, пробуем снова
    Waiting,
    Active,
}

pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

pub fn build(p: &Profile, start: i64) -> Activity<'_> {
    let mut a = Activity::new().timestamps(Timestamps::new().start(start));
    if let Some(v) = &p.name {
        a = a.name(v.as_str());
    }
    if let Some(v) = &p.details {
        a = a.details(v.as_str());
    }
    if let Some(v) = &p.state {
        a = a.state(v.as_str());
    }
    let mut assets = Assets::new();
    let mut has_assets = false;
    if let Some(v) = &p.large_image {
        assets = assets.large_image(v.as_str());
        has_assets = true;
    }
    if let Some(v) = &p.large_text {
        assets = assets.large_text(v.as_str());
        has_assets = true;
    }
    if let Some(v) = &p.small_image {
        assets = assets.small_image(v.as_str());
        has_assets = true;
    }
    if let Some(v) = &p.small_text {
        assets = assets.small_text(v.as_str());
        has_assets = true;
    }
    if has_assets {
        a = a.assets(assets);
    }
    a
}

// спим кусками, чтобы stop срабатывал быстро
fn nap(stop: &AtomicBool, secs: u64) {
    for _ in 0..(secs * 5) {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        sleep(Duration::from_millis(200));
    }
}

// фоновый поток держит статус и переподключается, если дискорд перезапустили
pub struct Worker {
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<Status>>,
    handle: Option<JoinHandle<()>>,
}

impl Worker {
    pub fn spawn(p: Profile, start: i64) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let status = Arc::new(Mutex::new(Status::Waiting));
        let (s2, st2) = (stop.clone(), status.clone());
        let handle = thread::spawn(move || run(p, start, s2, st2));
        Worker { stop, status, handle: Some(handle) }
    }

    pub fn status(&self) -> Status {
        *self.status.lock().unwrap()
    }

    // убирает статус из дискорда и ждёт завершения потока
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.stop();
    }
}

fn set_status(status: &Mutex<Status>, s: Status) {
    *status.lock().unwrap() = s;
}

fn run(p: Profile, start: i64, stop: Arc<AtomicBool>, status: Arc<Mutex<Status>>) {
    let mut client = DiscordIpcClient::new(&p.app_id);
    let mut connected = false;
    let mut tried = false;

    while !stop.load(Ordering::Relaxed) {
        if !connected {
            let r = if tried { client.reconnect() } else { client.connect() };
            tried = true;
            if r.is_err() {
                set_status(&status, Status::Waiting);
                nap(&stop, 3);
                continue;
            }
            connected = true;
        }
        // шлём заново каждые 15 с: лимит дискорда и заодно проверка связи
        if client.set_activity(build(&p, start)).is_ok() {
            set_status(&status, Status::Active);
            nap(&stop, 15);
        } else {
            connected = false;
            set_status(&status, Status::Waiting);
        }
    }
    if connected {
        let _ = client.clear_activity();
        let _ = client.close();
    }
}
