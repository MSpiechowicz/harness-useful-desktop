//! The launch sound, played natively: the Linux webview (WebKitGTK) keeps Web Audio suspended until
//! a user gesture and Tauri cannot lift that, while any gesture on the splash skips it. The track is
//! the short logo's soundtrack from ../harness-useful-logo (`renders/desktop/video/short.webm`), so it
//! is timed to the animation the webview starts at the same moment.
//!
//! Opening the audio device takes a noticeable moment, so [`warm_up`] opens it in the background at
//! startup and the sound starts the instant the intro asks. One worker thread owns the device (the OS
//! stream is not `Send`); it closes the device and ends once it has been idle for [`IDLE`], and the
//! next request starts it again. Nothing here blocks or fails the app: without an audio device the
//! sound is skipped with a warning.
//!
//! The same worker plays the chat alert chime ([`play_alert_chime`]), a short two-note sound made for
//! this app. Whether it may play is the caller's decision, not this module's.

use std::io::Cursor;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player};

/// Mono 48 kHz PCM, about 2.9 s.
const LAUNCH: &[u8] = include_bytes!("../../sounds/launch.wav");

/// Mono 48 kHz PCM, about 0.4 s.
const ALERT: &[u8] = include_bytes!("../../sounds/alert.wav");

/// How long the worker keeps the device open without anything to play.
const IDLE: Duration = Duration::from_secs(15);

#[derive(Clone, Copy)]
enum Request {
    Play,
    Chime,
    Stop,
}

impl Request {
    /// The sound the request starts; none for a stop.
    fn track(&self) -> Option<&'static [u8]> {
        match self {
            Self::Play => Some(LAUNCH),
            Self::Chime => Some(ALERT),
            Self::Stop => None,
        }
    }
}

/// The running worker's inbox, if any.
static WORKER: Mutex<Option<Sender<Request>>> = Mutex::new(None);

/// Opens the audio device in the background, so the first [`launch_sound`] plays without delay.
pub fn warm_up() {
    let mut worker = WORKER.lock().unwrap_or_else(PoisonError::into_inner);

    if worker.is_none() {
        *worker = spawn_worker();
    }
}

/// Starts the launch sound (`play: true`, restarting one already playing) or stops it (`false`,
/// when the intro is skipped).
#[tauri::command]
pub fn launch_sound(play: bool) {
    send(if play { Request::Play } else { Request::Stop });
}

/// Plays the chat alert chime, replacing a sound that is still playing. Returns at once.
pub fn play_alert_chime() {
    send(Request::Chime);
}

/// Hands `request` to the worker, starting one that went idle (or never started). Stopping needs no
/// worker.
fn send(request: Request) {
    let mut worker = WORKER.lock().unwrap_or_else(PoisonError::into_inner);

    let delivered = worker.as_ref().is_some_and(|inbox| inbox.send(request).is_ok());

    if !delivered && request.track().is_some() {
        *worker = spawn_worker();

        if let Some(inbox) = worker.as_ref() {
            let _ = inbox.send(request);
        }
    }
}

fn spawn_worker() -> Option<Sender<Request>> {
    let (inbox, requests) = mpsc::channel();

    let spawned = thread::Builder::new().name("launch-sound".into()).spawn(move || {
        let sink = match DeviceSinkBuilder::open_default_sink() {
            Ok(mut sink) => {
                sink.log_on_drop(false);
                sink
            }
            Err(error) => {
                tracing::warn!(%error, "no audio device, sounds are skipped");
                return;
            }
        };

        serve(&sink, &requests);
    });

    match spawned {
        Ok(_) => Some(inbox),
        Err(error) => {
            tracing::warn!(%error, "cannot start the sound worker");
            None
        }
    }
}

/// Plays and stops on request until the device has been idle for [`IDLE`].
fn serve(sink: &MixerDeviceSink, requests: &mpsc::Receiver<Request>) {
    let mut player: Option<Player> = None;

    loop {
        let request = match requests.recv_timeout(IDLE) {
            Ok(request) => request,
            Err(RecvTimeoutError::Disconnected) => return,
            Err(RecvTimeoutError::Timeout) if player.as_ref().is_some_and(|playing| !playing.empty()) => continue,
            Err(RecvTimeoutError::Timeout) => {
                // Retire under the lock, so no request can slip into this inbox after the last look at it.
                let mut worker = WORKER.lock().unwrap_or_else(PoisonError::into_inner);
                match requests.try_recv() {
                    Ok(request) => request,
                    Err(_) => {
                        *worker = None;
                        return;
                    }
                }
            }
        };

        if let Some(previous) = player.take() {
            previous.stop();
        }

        if let Some(bytes) = request.track() {
            match Decoder::try_from(Cursor::new(bytes)) {
                Ok(track) => {
                    let next = Player::connect_new(sink.mixer());
                    next.append(track);
                    player = Some(next);
                }
                Err(error) => tracing::warn!(%error, "cannot decode a sound"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ALERT, LAUNCH};

    #[test]
    fn the_launch_track_decodes() {
        let decoder = rodio::Decoder::try_from(std::io::Cursor::new(LAUNCH)).expect("valid WAV");
        let duration = rodio::Source::total_duration(&decoder).expect("known length");

        assert!(
            duration.as_secs_f32() > 1.9 && duration.as_secs_f32() < 4.0,
            "{duration:?}"
        );
    }

    #[test]
    fn the_alert_chime_decodes_and_is_short() {
        let decoder = rodio::Decoder::try_from(std::io::Cursor::new(ALERT)).expect("valid WAV");
        let duration = rodio::Source::total_duration(&decoder).expect("known length");

        assert!(
            duration.as_secs_f32() > 0.2 && duration.as_secs_f32() < 0.8,
            "{duration:?}"
        );
    }
}
