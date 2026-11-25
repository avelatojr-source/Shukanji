//! Pure TikTok LIVE monitoring types. No IO, no GUI, no external crates.
//!
//! The `integrations::tiktok_live` module translates the library's protobuf
//! events into these small, owned structs so the domain stays dependency-free.

use std::collections::VecDeque;

/// Connection lifecycle of the LIVE monitor.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LiveConnectionState {
    #[default]
    Disconnected,
    Connecting {
        username: String,
    },
    Connected {
        username: String,
        room_id: String,
    },
    Reconnecting {
        attempt: u32,
        max_retries: u32,
    },
    Failed {
        message: String,
    },
}

impl LiveConnectionState {
    pub fn label(&self) -> &'static str {
        match self {
            LiveConnectionState::Disconnected => "Monitor off",
            LiveConnectionState::Connecting { .. } => "Connecting…",
            LiveConnectionState::Connected { .. } => "Monitoring",
            LiveConnectionState::Reconnecting { .. } => "Reconnecting…",
            LiveConnectionState::Failed { .. } => "Monitor failed",
        }
    }

    pub fn is_connected(&self) -> bool {
        matches!(self, LiveConnectionState::Connected { .. })
    }
}

/// A custom emote placed inside a chat comment.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChatEmote {
    /// UTF-16 code-unit offset of the emote within the comment
    /// (`place_in_comment` from TikTok).
    pub position: usize,
    /// Real TikTok emote image URL.
    pub url: String,
}

/// A single chat comment.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChatLine {
    /// Arrival order, assigned by [`LiveMonitorData::apply`]. Lines are shown in
    /// this order so the newest activity is always at the bottom of the chat.
    pub seq: u64,
    pub user: String,
    pub message: String,
    pub verified: bool,
    pub subscriber: bool,
    /// Real TikTok profile picture URL (never a placeholder).
    pub avatar_url: Option<String>,
    /// Custom emotes to splice into the comment text.
    pub emotes: Vec<ChatEmote>,
}

/// A gift (or gift-streak update).
///
/// Combo gifts fire several events per streak; the integration tracks the
/// streak so the UI can replace in place instead of duplicating lines.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GiftLine {
    /// Arrival order (see [`ChatLine::seq`]).
    pub seq: u64,
    /// Stable id for a combo streak (TikTok `group_id`); shared by all events
    /// of the same streak.
    pub streak_id: u64,
    pub user: String,
    pub gift: String,
    /// TikTok gift id (`0` when unknown).
    pub gift_id: i64,
    /// Total quantity in this streak so far.
    pub count: i32,
    /// Total diamonds for this streak so far.
    pub diamonds: i64,
    /// Diamonds attributable to this individual event (the delta).
    pub delta_diamonds: i64,
    pub combo: bool,
    /// True once the streak is over (or for single-fire gifts).
    pub final_streak: bool,
    /// Real TikTok gift artwork URL (static, GIF or WebP).
    pub image_url: Option<String>,
}

/// A follow / share / join / like.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActivityKind {
    #[default]
    Follow,
    Share,
    Join,
    /// A viewer tapped the heart. Coalesced per user by the integration, so
    /// `count` is how many taps that one line represents.
    Like,
}

impl ActivityKind {
    pub fn label(self) -> &'static str {
        match self {
            ActivityKind::Follow => "followed",
            ActivityKind::Share => "shared",
            ActivityKind::Join => "joined",
            ActivityKind::Like => "liked",
        }
    }
}

/// A single activity entry.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ActivityLine {
    /// Arrival order (see [`ChatLine::seq`]).
    pub seq: u64,
    pub kind: ActivityKind,
    pub user: String,
    /// Real TikTok profile picture URL (never a placeholder).
    pub avatar_url: Option<String>,
    /// Heart taps this line stands for (0 for follow/share/join).
    pub count: i32,
}

/// Everything the monitor task sends to the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonitorEvent {
    Chat(ChatLine),
    Gift(GiftLine),
    Activity(ActivityLine),
    /// Current room counters (from `RoomUserSeq`).
    ViewerCount {
        viewers: i64,
        total: i64,
        anonymous: i64,
    },
    /// Monotonized like total.
    Likes {
        total: i64,
    },
    /// The host ended the LIVE.
    LiveEnded,
}

/// Ring-buffer sizes for the monitor feeds.
pub const MAX_CHAT_LINES: usize = 300;
pub const MAX_GIFT_LINES: usize = 100;
pub const MAX_ACTIVITY_LINES: usize = 100;

/// Accumulated monitor state: capped feeds plus running counters.
///
/// This is pure and unit-tested, including the combo-gift de-duplication
/// (an ongoing streak updates in place rather than appending a new line) and
/// the delta-based diamond total.
#[derive(Debug, Clone, Default)]
pub struct LiveMonitorData {
    pub chat: VecDeque<ChatLine>,
    pub gifts: VecDeque<GiftLine>,
    pub activity: VecDeque<ActivityLine>,
    pub viewers: i64,
    pub total_viewers: i64,
    pub anonymous: i64,
    pub likes: i64,
    pub diamonds: i64,
    pub follows: u64,
    pub shares: u64,
    pub joins: u64,
    /// Monotonic arrival counter handed out to every line.
    next_seq: u64,
}

/// Where one unified-feed line lives: which buffer, and which index in it.
///
/// Deliberately a plain owned pair rather than a borrow, so the UI can keep one
/// buffer and refill it every frame instead of allocating a fresh, sorted list
/// of the whole feed 60 times a second.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeedEntry {
    pub kind: FeedKind,
    pub index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedKind {
    Chat,
    Gift,
    Activity,
}

impl LiveMonitorData {
    /// Resolve a feed entry back to its line.
    pub fn chat_at(&self, index: usize) -> Option<&ChatLine> {
        self.chat.get(index)
    }
    pub fn gift_at(&self, index: usize) -> Option<&GiftLine> {
        self.gifts.get(index)
    }
    pub fn activity_at(&self, index: usize) -> Option<&ActivityLine> {
        self.activity.get(index)
    }
}

impl LiveMonitorData {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// A new arrival stamp. Every pushed line gets one so the UI can show a
    /// single chronological feed instead of chat-then-activity-then-gifts.
    fn take_seq(&mut self) -> u64 {
        self.next_seq += 1;
        self.next_seq
    }

    /// Chat / activity / gifts merged into arrival order, written into a
    /// caller-owned buffer (cleared first) so the UI can reuse one buffer every
    /// frame instead of allocating and sorting the whole feed 60 times a second.
    ///
    /// Each buffer is already in arrival order, so this is a three-way merge:
    /// O(n), no sort, no allocation when the buffer is already big enough. The
    /// tab switches choose which kinds are involved.
    pub fn timeline_into(&self, chat: bool, activity: bool, gifts: bool, out: &mut Vec<FeedEntry>) {
        out.clear();
        out.reserve(self.chat.len() + self.activity.len() + self.gifts.len());

        let (mut next_chat, mut next_gift, mut next_activity) = (0usize, 0usize, 0usize);
        loop {
            let mut best: Option<(u64, FeedEntry)> = None;
            let consider = |seq: u64, entry: FeedEntry, best: &mut Option<(u64, FeedEntry)>| {
                if best.is_none_or(|(best_seq, _)| seq < best_seq) {
                    *best = Some((seq, entry));
                }
            };
            if chat && let Some(line) = self.chat.get(next_chat) {
                consider(
                    line.seq,
                    FeedEntry {
                        kind: FeedKind::Chat,
                        index: next_chat,
                    },
                    &mut best,
                );
            }
            if activity && let Some(line) = self.activity.get(next_activity) {
                consider(
                    line.seq,
                    FeedEntry {
                        kind: FeedKind::Activity,
                        index: next_activity,
                    },
                    &mut best,
                );
            }
            if gifts && let Some(line) = self.gifts.get(next_gift) {
                consider(
                    line.seq,
                    FeedEntry {
                        kind: FeedKind::Gift,
                        index: next_gift,
                    },
                    &mut best,
                );
            }
            let Some((_, entry)) = best else {
                break;
            };
            match entry.kind {
                FeedKind::Chat => next_chat += 1,
                FeedKind::Gift => next_gift += 1,
                FeedKind::Activity => next_activity += 1,
            }
            out.push(entry);
        }
    }

    /// Fold one monitor event into the accumulated state.
    pub fn apply(&mut self, event: MonitorEvent) {
        match event {
            MonitorEvent::Chat(mut line) => {
                line.seq = self.take_seq();
                push_capped(&mut self.chat, line, MAX_CHAT_LINES)
            }
            MonitorEvent::Gift(mut line) => {
                // Deltas sum to the correct running total even when a combo
                // streak is updated in place.
                self.diamonds += line.delta_diamonds.max(0);
                if let Some(existing) = self
                    .gifts
                    .iter_mut()
                    .rev()
                    .find(|g| g.streak_id == line.streak_id && !g.final_streak)
                {
                    // Keep the streak's original position in the feed.
                    line.seq = existing.seq;
                    *existing = line;
                } else {
                    line.seq = self.take_seq();
                    push_capped(&mut self.gifts, line, MAX_GIFT_LINES);
                }
            }
            MonitorEvent::Activity(mut line) => {
                match line.kind {
                    ActivityKind::Follow => self.follows += 1,
                    ActivityKind::Share => self.shares += 1,
                    ActivityKind::Join => self.joins += 1,
                    // Likes have their own monotonized counter.
                    ActivityKind::Like => {}
                }
                line.seq = self.take_seq();
                push_capped(&mut self.activity, line, MAX_ACTIVITY_LINES);
            }
            MonitorEvent::ViewerCount {
                viewers,
                total,
                anonymous,
            } => {
                self.viewers = viewers;
                self.total_viewers = total;
                self.anonymous = anonymous;
            }
            MonitorEvent::Likes { total } => {
                self.likes = self.likes.max(total);
            }
            MonitorEvent::LiveEnded => {}
        }
    }
}

fn push_capped<T>(buffer: &mut VecDeque<T>, item: T, cap: usize) {
    buffer.push_back(item);
    while buffer.len() > cap {
        buffer.pop_front();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gift(streak_id: u64, delta: i64, total: i64, count: i32, final_streak: bool) -> GiftLine {
        GiftLine {
            streak_id,
            user: "alice".to_string(),
            gift: "Rose".to_string(),
            gift_id: 1,
            count,
            diamonds: total,
            delta_diamonds: delta,
            combo: true,
            final_streak,
            image_url: None,
            ..Default::default()
        }
    }

    fn chat(message: &str) -> MonitorEvent {
        MonitorEvent::Chat(ChatLine {
            user: "u".into(),
            message: message.into(),
            ..Default::default()
        })
    }

    fn activity(kind: ActivityKind, user: &str) -> MonitorEvent {
        MonitorEvent::Activity(ActivityLine {
            kind,
            user: user.into(),
            ..Default::default()
        })
    }

    /// Render the merged feed as readable strings, reusing one buffer.
    fn order_of(data: &LiveMonitorData, chat: bool, activity: bool, gifts: bool) -> Vec<String> {
        let mut buffer = Vec::new();
        data.timeline_into(chat, activity, gifts, &mut buffer);
        buffer
            .iter()
            .map(|entry| match entry.kind {
                FeedKind::Chat => format!(
                    "chat:{}",
                    data.chat_at(entry.index).expect("chat index").message
                ),
                FeedKind::Activity => {
                    let line = data.activity_at(entry.index).expect("activity index");
                    format!("{}:{}", line.kind.label(), line.user)
                }
                FeedKind::Gift => format!(
                    "gift:{}",
                    data.gift_at(entry.index).expect("gift index").gift
                ),
            })
            .collect()
    }

    #[test]
    fn the_feed_is_in_arrival_order_across_kinds() {
        let mut data = LiveMonitorData::default();
        data.apply(chat("first"));
        data.apply(activity(ActivityKind::Follow, "bob"));
        data.apply(chat("second"));
        data.apply(activity(ActivityKind::Like, "carol"));
        data.apply(MonitorEvent::Gift(gift(1, 5, 5, 1, true)));

        assert_eq!(
            order_of(&data, true, true, true),
            vec![
                "chat:first",
                "followed:bob",
                "chat:second",
                "liked:carol",
                "gift:Rose"
            ]
        );
    }

    #[test]
    fn the_merge_matches_a_full_sort() {
        // The three-way merge must produce exactly the same order as sorting
        // every line by arrival stamp.
        let mut data = LiveMonitorData::default();
        for i in 0..40 {
            match i % 4 {
                0 => data.apply(chat(&format!("c{i}"))),
                1 => data.apply(activity(ActivityKind::Follow, &format!("u{i}"))),
                2 => data.apply(MonitorEvent::Gift(gift(i as u64, 1, 1, 1, true))),
                _ => data.apply(activity(ActivityKind::Like, &format!("u{i}"))),
            }
        }
        let merged = order_of(&data, true, true, true);

        let mut expected: Vec<(u64, String)> = Vec::new();
        expected.extend(
            data.chat
                .iter()
                .map(|l| (l.seq, format!("chat:{}", l.message))),
        );
        expected.extend(
            data.gifts
                .iter()
                .map(|l| (l.seq, format!("gift:{}", l.gift))),
        );
        expected.extend(
            data.activity
                .iter()
                .map(|l| (l.seq, format!("{}:{}", l.kind.label(), l.user))),
        );
        expected.sort_by_key(|(seq, _)| *seq);

        assert_eq!(
            merged,
            expected.into_iter().map(|(_, s)| s).collect::<Vec<_>>()
        );
    }

    #[test]
    fn the_timeline_honours_the_tab_filters_and_reuses_its_buffer() {
        let mut data = LiveMonitorData::default();
        data.apply(chat("hey"));
        data.apply(activity(ActivityKind::Share, "bob"));
        data.apply(MonitorEvent::Gift(gift(1, 5, 5, 1, true)));

        let mut buffer = Vec::new();
        data.timeline_into(true, true, false, &mut buffer);
        assert_eq!(buffer.len(), 2);

        // Refilling clears the previous contents rather than appending.
        data.timeline_into(false, false, true, &mut buffer);
        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer[0].kind, FeedKind::Gift);

        data.timeline_into(true, false, false, &mut buffer);
        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer[0].kind, FeedKind::Chat);
    }

    #[test]
    fn a_combo_streak_keeps_its_place_in_the_feed() {
        let mut data = LiveMonitorData::default();
        data.apply(MonitorEvent::Gift(gift(7, 1, 1, 1, false)));
        let first_seq = data.gifts[0].seq;
        data.apply(chat("after"));
        // More of the same streak: same position, still one line.
        data.apply(MonitorEvent::Gift(gift(7, 2, 3, 3, false)));
        assert_eq!(data.gifts.len(), 1);
        assert_eq!(data.gifts[0].seq, first_seq);
        assert_eq!(data.diamonds, 3);
    }

    #[test]
    fn labels_and_connection_flag() {
        assert_eq!(LiveConnectionState::Disconnected.label(), "Monitor off");
        assert!(!LiveConnectionState::Disconnected.is_connected());
        assert!(
            LiveConnectionState::Connected {
                username: "a".into(),
                room_id: "1".into()
            }
            .is_connected()
        );
    }

    #[test]
    fn combo_streak_updates_in_place_and_sums_deltas() {
        let mut data = LiveMonitorData::default();
        data.apply(MonitorEvent::Gift(gift(7, 10, 10, 1, false)));
        assert_eq!(data.gifts.len(), 1);
        assert_eq!(data.diamonds, 10);

        // Same streak, more gifts: still one line, delta added.
        data.apply(MonitorEvent::Gift(gift(7, 20, 30, 3, false)));
        assert_eq!(data.gifts.len(), 1);
        assert_eq!(data.diamonds, 30);
        assert_eq!(data.gifts[0].count, 3);
        assert_eq!(data.gifts[0].diamonds, 30);

        // Streak over: the line is finalised in place, not duplicated.
        data.apply(MonitorEvent::Gift(gift(7, 10, 40, 4, true)));
        assert_eq!(data.gifts.len(), 1);
        assert!(data.gifts[0].final_streak);
        assert_eq!(data.diamonds, 40);
    }

    #[test]
    fn distinct_single_gifts_append() {
        let mut data = LiveMonitorData::default();
        data.apply(MonitorEvent::Gift(gift(0, 5, 5, 1, true)));
        data.apply(MonitorEvent::Gift(gift(0, 5, 5, 1, true)));
        assert_eq!(data.gifts.len(), 2);
        assert_eq!(data.diamonds, 10);
    }

    #[test]
    fn chat_is_capped() {
        let mut data = LiveMonitorData::default();
        for i in 0..(MAX_CHAT_LINES + 25) {
            data.apply(chat(&i.to_string()));
        }
        assert_eq!(data.chat.len(), MAX_CHAT_LINES);
    }

    #[test]
    fn activity_counters_and_likes_and_viewers() {
        let mut data = LiveMonitorData::default();
        for kind in [
            ActivityKind::Follow,
            ActivityKind::Follow,
            ActivityKind::Share,
            ActivityKind::Join,
        ] {
            data.apply(activity(kind, "bob"));
        }
        assert_eq!((data.follows, data.shares, data.joins), (2, 1, 1));

        // Likes are counted by the monotonized total, not by kind.
        data.apply(activity(ActivityKind::Like, "carol"));
        assert_eq!((data.follows, data.shares, data.joins), (2, 1, 1));

        data.apply(MonitorEvent::Likes { total: 100 });
        data.apply(MonitorEvent::Likes { total: 80 }); // stale shard: ignored
        assert_eq!(data.likes, 100);

        data.apply(MonitorEvent::ViewerCount {
            viewers: 12,
            total: 40,
            anonymous: 28,
        });
        assert_eq!((data.viewers, data.total_viewers), (12, 40));
    }

    #[test]
    fn reset_clears_everything() {
        let mut data = LiveMonitorData::default();
        data.apply(MonitorEvent::Likes { total: 5 });
        data.apply(MonitorEvent::Gift(gift(1, 3, 3, 1, true)));
        data.reset();
        assert_eq!(data.likes, 0);
        assert!(data.gifts.is_empty());
        assert_eq!(data.diamonds, 0);
    }
}
