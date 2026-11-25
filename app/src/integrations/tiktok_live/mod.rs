//! TikTok LIVE monitoring, backed by the `piratetok-live-rs` crate.
//!
//! A single background task owns one [`TikTokLiveStream`] and translates the
//! library's decoded protobuf events into our dependency-free
//! [`MonitorEvent`]s. Gift streaks are de-duplicated with the library's
//! [`GiftStreakTracker`] (combo gifts fire many events per streak), and likes
//! are monotonized with [`LikeAccumulator`] and throttled before reaching the
//! UI so a like flood cannot swamp the event channel.
//!
//! Reconnection is handled inside the library (stale detection + backoff); we
//! only surface the `Reconnecting`/`Disconnected` lifecycle.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use piratetok_live_rs::TikTokLive;
use piratetok_live_rs::errors::TikTokLiveError;
use piratetok_live_rs::helpers::gift_streak::GiftStreakTracker;
use piratetok_live_rs::helpers::like_accumulator::LikeAccumulator;
use piratetok_live_rs::structs::TikTokLiveEvent;
use piratetok_live_rs::structs::proto::UserIdentity;

use crate::domain::events::{AppEvent, EventSink};
use crate::domain::monitor::{
    ActivityKind, ActivityLine, ChatEmote, ChatLine, GiftLine, LiveConnectionState, MonitorEvent,
};

/// Whether a membership message's description reads as a join. Used to catch
/// re-joins that the library does not classify as `action == 1`. Deliberately
/// narrow: "left"/"leave" descriptions must not count.
pub fn looks_like_join(action_description: &str) -> bool {
    let text = action_description.trim().to_ascii_lowercase();
    if text.is_empty() || text.contains("left") || text.contains("leave") {
        return false;
    }
    text.contains("join") || text.contains("entered") || text.contains("came in")
}

/// Emit the monotonized like *total* at most this often (TikTok sends a like
/// event per tap).
const LIKE_EMIT_INTERVAL: Duration = Duration::from_millis(500);
/// A viewer's taps are coalesced into one "X liked xN" feed line per user per
/// this window, so a like flood cannot swamp the chat.
const LIKE_LINE_WINDOW: Duration = Duration::from_secs(3);
/// Drop per-user like state that has been idle this long.
const LIKE_STATE_TTL: Duration = Duration::from_secs(60);

/// A handle to a running monitor task. Dropping it does **not** stop the task;
/// call [`LiveMonitorHandle::shutdown`] (used when replacing/disconnecting).
pub struct LiveMonitorHandle {
    abort: tokio::task::AbortHandle,
}

impl LiveMonitorHandle {
    pub fn shutdown(&self) {
        self.abort.abort();
    }
}

/// Spawn a monitor task for `username`. Automatically stops any previously
/// running monitor when called via the coordinator.
pub fn spawn_monitor(username: String, sink: Arc<dyn EventSink>) -> LiveMonitorHandle {
    let task = tokio::spawn(run(username, sink));
    LiveMonitorHandle {
        abort: task.abort_handle(),
    }
}

async fn run(username: String, sink: Arc<dyn EventSink>) {
    sink.emit(AppEvent::LiveMonitorStatus {
        state: LiveConnectionState::Connecting {
            username: username.clone(),
        },
    });

    let mut stream = match TikTokLive::builder(&username).connect().await {
        Ok(stream) => stream,
        Err(e) => {
            sink.emit(AppEvent::LiveMonitorStatus {
                state: LiveConnectionState::Failed {
                    message: friendly(&e),
                },
            });
            return;
        }
    };

    let mut gifts = GiftStreakTracker::new();
    let mut likes = LikeAccumulator::new();
    let mut last_like_emit = Instant::now() - LIKE_EMIT_INTERVAL;
    // Per-user: (window start, taps collected during this window).
    let mut like_windows: HashMap<String, (Instant, i32)> = HashMap::new();

    loop {
        let event = match stream.next_event().await {
            Ok(event) => event,
            Err(e) => {
                sink.emit(AppEvent::LiveMonitorStatus {
                    state: LiveConnectionState::Failed {
                        message: friendly(&e),
                    },
                });
                return;
            }
        };

        match event {
            TikTokLiveEvent::Connected { room_id } => {
                sink.emit(AppEvent::LiveMonitorStatus {
                    state: LiveConnectionState::Connected {
                        username: username.clone(),
                        room_id,
                    },
                });
            }
            TikTokLiveEvent::Reconnecting {
                attempt,
                max_retries,
                ..
            } => {
                sink.emit(AppEvent::LiveMonitorStatus {
                    state: LiveConnectionState::Reconnecting {
                        attempt,
                        max_retries,
                    },
                });
            }
            TikTokLiveEvent::Disconnected => {
                sink.emit(AppEvent::LiveMonitorStatus {
                    state: LiveConnectionState::Disconnected,
                });
                return;
            }
            TikTokLiveEvent::Chat(msg) => {
                let bits = user_bits(&msg.user);
                let emotes = msg
                    .emotes
                    .iter()
                    .filter_map(|emote| {
                        let url = emote
                            .emote
                            .as_ref()?
                            .image
                            .as_ref()?
                            .url_list
                            .first()
                            .cloned()?;
                        Some(ChatEmote {
                            position: emote.place_in_comment.max(0) as usize,
                            url,
                        })
                    })
                    .collect();
                sink.emit(AppEvent::LiveMonitorEvent {
                    event: MonitorEvent::Chat(ChatLine {
                        user: bits.name,
                        message: msg.comment,
                        verified: bits.verified,
                        subscriber: bits.subscriber,
                        avatar_url: bits.avatar,
                        emotes,
                        ..Default::default()
                    }),
                });
            }
            TikTokLiveEvent::Gift(msg) => {
                let streak = gifts.process(&msg);
                let bits = user_bits(&msg.user);
                let details = msg.gift_details.as_ref();
                let gift_name = details
                    .map(|g| g.gift_name.clone())
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| "gift".to_string());
                let gift_id = details.map(|g| g.id).unwrap_or(0);
                let image_url = details
                    .and_then(|g| g.gift_image.as_ref())
                    .and_then(|img| img.url_list.first().cloned());
                sink.emit(AppEvent::LiveMonitorEvent {
                    event: MonitorEvent::Gift(GiftLine {
                        streak_id: streak.streak_id,
                        user: bits.name,
                        gift: gift_name,
                        gift_id,
                        count: streak.total_gift_count,
                        diamonds: streak.total_diamond_count,
                        delta_diamonds: streak.event_diamond_count,
                        combo: msg.is_combo_gift(),
                        final_streak: streak.is_final,
                        image_url,
                        ..Default::default()
                    }),
                });
            }
            TikTokLiveEvent::Like(msg) => {
                let stats = likes.process(&msg);
                if last_like_emit.elapsed() >= LIKE_EMIT_INTERVAL {
                    last_like_emit = Instant::now();
                    sink.emit(AppEvent::LiveMonitorEvent {
                        event: MonitorEvent::Likes {
                            total: stats.total_like_count,
                        },
                    });
                }

                // Coalesce this viewer's taps into one feed line per window so
                // the chat shows *who* liked without drowning in taps.
                let bits = user_bits(&msg.user);
                let taps = msg.like_count.max(1);
                let now = Instant::now();
                if like_windows.len() > 512 {
                    like_windows
                        .retain(|_, (start, _)| now.duration_since(*start) < LIKE_STATE_TTL);
                }
                match like_windows.get_mut(&bits.name) {
                    Some((start, collected)) => {
                        *collected += taps;
                        if now.duration_since(*start) >= LIKE_LINE_WINDOW {
                            let count = std::mem::take(collected);
                            *start = now;
                            sink.emit(AppEvent::LiveMonitorEvent {
                                event: MonitorEvent::Activity(ActivityLine {
                                    kind: ActivityKind::Like,
                                    user: bits.name,
                                    avatar_url: bits.avatar,
                                    count,
                                    ..Default::default()
                                }),
                            });
                        }
                    }
                    None => {
                        // First tap: show it immediately.
                        like_windows.insert(bits.name.clone(), (now, 0));
                        sink.emit(AppEvent::LiveMonitorEvent {
                            event: MonitorEvent::Activity(ActivityLine {
                                kind: ActivityKind::Like,
                                user: bits.name,
                                avatar_url: bits.avatar,
                                count: taps,
                                ..Default::default()
                            }),
                        });
                    }
                }
            }
            TikTokLiveEvent::Follow(msg) => {
                let bits = user_bits(&msg.user);
                sink.emit(AppEvent::LiveMonitorEvent {
                    event: MonitorEvent::Activity(ActivityLine {
                        kind: ActivityKind::Follow,
                        user: bits.name,
                        avatar_url: bits.avatar,
                        ..Default::default()
                    }),
                });
            }
            TikTokLiveEvent::Share(msg) => {
                let bits = user_bits(&msg.user);
                sink.emit(AppEvent::LiveMonitorEvent {
                    event: MonitorEvent::Activity(ActivityLine {
                        kind: ActivityKind::Share,
                        user: bits.name,
                        avatar_url: bits.avatar,
                        ..Default::default()
                    }),
                });
            }
            TikTokLiveEvent::Join(msg) => {
                // Every join is reported, including a viewer who rejoins, so a
                // repeat join shows up as another line in the feed. There is no
                // per-user de-duplication anywhere in this path.
                let bits = user_bits(&msg.user);
                sink.emit(AppEvent::LiveMonitorEvent {
                    event: MonitorEvent::Activity(ActivityLine {
                        kind: ActivityKind::Join,
                        user: bits.name,
                        avatar_url: bits.avatar,
                        ..Default::default()
                    }),
                });
            }
            TikTokLiveEvent::Member(msg) => {
                // The library only raises `Join` for `action == 1`. TikTok
                // sometimes re-sends the membership message for a viewer who
                // already joined (a leave-and-return, or a second device) with a
                // different action code but a "joined" description. Catch those
                // too so the join repeats instead of being silently dropped.
                // `action == 1` is skipped here because `Join` already fired.
                if msg.action != 1 && looks_like_join(&msg.action_description) {
                    let bits = user_bits(&msg.user);
                    sink.emit(AppEvent::LiveMonitorEvent {
                        event: MonitorEvent::Activity(ActivityLine {
                            kind: ActivityKind::Join,
                            user: bits.name,
                            avatar_url: bits.avatar,
                            ..Default::default()
                        }),
                    });
                }
            }
            TikTokLiveEvent::RoomUserSeq(msg) => {
                sink.emit(AppEvent::LiveMonitorEvent {
                    event: MonitorEvent::ViewerCount {
                        viewers: msg.viewer_count,
                        total: msg.total_user,
                        anonymous: msg.anonymous,
                    },
                });
            }
            TikTokLiveEvent::LiveEnded(_) => {
                sink.emit(AppEvent::LiveMonitorEvent {
                    event: MonitorEvent::LiveEnded,
                });
            }
            // The remaining decoded event types are intentionally ignored.
            _ => {}
        }
    }
}

/// Display name, chat flags and real avatar URL for a user.
struct UserBits {
    name: String,
    verified: bool,
    subscriber: bool,
    avatar: Option<String>,
}

fn user_bits(user: &Option<UserIdentity>) -> UserBits {
    match user {
        Some(user) => {
            let name = if user.nickname.trim().is_empty() {
                user.unique_id.clone()
            } else {
                user.nickname.clone()
            };
            let avatar = user
                .avatar_thumb
                .as_ref()
                .or(user.avatar_medium.as_ref())
                .or(user.avatar_large.as_ref())
                .and_then(|img| img.url_list.first().cloned());
            UserBits {
                name,
                verified: user.verified,
                subscriber: user.fans_club.is_some(),
                avatar,
            }
        }
        None => UserBits {
            name: "unknown".to_string(),
            verified: false,
            subscriber: false,
            avatar: None,
        },
    }
}

/// Human-friendly message for connection errors (never contains secrets).
fn friendly(error: &TikTokLiveError) -> String {
    match error {
        TikTokLiveError::UserNotFound(user) => format!("TikTok user not found: @{user}"),
        TikTokLiveError::HostNotOnline(user) => {
            format!("@{user} is not currently live. Go live first, then retry.")
        }
        TikTokLiveError::TikTokBlocked(reason) => {
            format!("TikTok blocked the request ({reason}). Try again shortly.")
        }
        TikTokLiveError::AgeRestricted(_) => {
            "That LIVE is age-restricted and needs session cookies.".to_string()
        }
        TikTokLiveError::DeviceBlocked => {
            "TikTok flagged this device (ttwid). Retrying with a fresh one…".to_string()
        }
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_descriptions_are_recognised_but_leaves_are_not() {
        for text in ["joined", "Joined", " has entered", "came in"] {
            assert!(looks_like_join(text), "{text:?} should count as a join");
        }
        for text in ["", "left", "has left the room", "leaved"] {
            assert!(!looks_like_join(text), "{text:?} must not count as a join");
        }
    }
    #[test]
    fn friendly_messages_are_human_and_have_no_secrets() {
        let not_live = friendly(&TikTokLiveError::HostNotOnline("bob".into()));
        assert!(not_live.contains("not currently live"), "{not_live}");

        let missing = friendly(&TikTokLiveError::UserNotFound("bob".into()));
        assert!(missing.contains("@bob"), "{missing}");
    }

    #[test]
    fn user_bits_prefers_nickname_then_unique_id() {
        let user = UserIdentity {
            nickname: "Alice".to_string(),
            unique_id: "alice123".to_string(),
            verified: true,
            avatar_thumb: Some(piratetok_live_rs::structs::proto::Image {
                url_list: vec!["https://cdn/avatar.png".to_string()],
                ..Default::default()
            }),
            ..Default::default()
        };
        let bits = user_bits(&Some(user));
        assert_eq!(bits.name, "Alice");
        assert!(bits.verified);
        assert!(!bits.subscriber);
        assert_eq!(bits.avatar.as_deref(), Some("https://cdn/avatar.png"));

        let blank = UserIdentity {
            unique_id: "fallback_handle".to_string(),
            ..Default::default()
        };
        assert_eq!(user_bits(&Some(blank)).name, "fallback_handle");

        assert_eq!(user_bits(&None).name, "unknown");
    }

    /// Opt-in: needs a real, currently-live TikTok account.
    ///
    /// Run with:
    /// `TIKTOK_LIVE_TEST_USERNAME=<handle> cargo test -- --ignored connects_to_a_real_live_room`
    #[tokio::test]
    #[ignore = "requires a live TikTok broadcast; set TIKTOK_LIVE_TEST_USERNAME"]
    async fn connects_to_a_real_live_room() {
        let Ok(username) = std::env::var("TIKTOK_LIVE_TEST_USERNAME") else {
            eprintln!("TIKTOK_LIVE_TEST_USERNAME not set; skipping");
            return;
        };
        let stream = TikTokLive::builder(&username).connect().await;
        assert!(
            stream.is_ok(),
            "connect failed: {}",
            stream.err().map(|e| e.to_string()).unwrap_or_default()
        );
    }
}
