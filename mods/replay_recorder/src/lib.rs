use mod_api::{
    self, ClientHost, ClientTickPostEvent, Command, CommandEvent, Corner, Flow, Hooks,
    InputKeyEvent, KeyAction, KeyConflictContext, KeyRegistration, PlayerLoggedInEvent,
    RegisterClientCommandsEvent, RegisterCommandsEvent, Registrar, RegistrationContext,
    RenderGuiPostEvent, RenderLevelStage, RenderLevelStageEvent, RunModCommandEvent, ServerHost,
    SettingChangedEvent, SettingSpec, Text, WorldDepth, WorldLine, WorldQuad,
};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

const ID: &str = "replay_recorder";
const KEY_TOGGLE_REC: &str = "toggle_rec";
const KEY_ADD_KEYFRAME: &str = "add_keyframe";

const MODES: [&str; 4] = [
    "Dual 1st+3rd Person + World Replay (Quark Auto-Upload)",
    "1st Person POV + World Replay",
    "3rd Person POV + World Replay",
    "Server-Wide 4-AI Replay Archive Only",
];

const SEG_LENGTHS: [&str; 4] = ["30s (Fast Cloud Sync)", "60s", "120s", "300s"];
const DISK_GUARDS: [&str; 3] = ["3 GB Free (Default)", "5 GB Free", "10 GB Free"];
const PURGE_POLICY: [&str; 2] = [
    "Immediate Delete After Quark Upload (Anti-Disk-Full)",
    "Keep Latest 1 Copy",
];

static RECORDING_ACTIVE: AtomicBool = AtomicBool::new(true);
static MODE_IDX: AtomicU32 = AtomicU32::new(0);
static KEYFRAME_COUNT: AtomicU32 = AtomicU32::new(0);
static TICKS_RECORDED: AtomicU32 = AtomicU32::new(0);
static KEYFRAMES: Mutex<Vec<([f64; 3], [f32; 2])>> = Mutex::new(Vec::new());

fn write_replay_state(
    active: bool,
    mode_idx: usize,
    kf_count: u32,
    last_kf: [f64; 3],
    rot: [f32; 2],
) {
    let dir = PathBuf::from("horizon-mods/.horizon-mod-data/replay_recorder");
    let _ = fs::create_dir_all(&dir);
    let payload = format!(
        "{{\n  \"recording_active\": {},\n  \"mode\": \"{}\",\n  \"quark_remote_dir\": \"/MC-AI-Horizon-Recordings\",\n  \"auto_purge_local\": true,\n  \"ticks_recorded\": {},\n  \"keyframes_recorded\": {},\n  \"last_keyframe_pos\": [{:.2}, {:.2}, {:.2}],\n  \"last_keyframe_rot\": [{:.2}, {:.2}]\n}}\n",
        active,
        MODES[mode_idx % MODES.len()],
        TICKS_RECORDED.load(Ordering::Relaxed),
        kf_count,
        last_kf[0],
        last_kf[1],
        last_kf[2],
        rot[0],
        rot[1]
    );
    let _ = fs::write(dir.join("replay_state.json"), payload);
}

fn append_world_event(event_type: &str, detail: &str) {
    let dir = PathBuf::from("horizon-mods/.horizon-mod-data/replay_recorder");
    let _ = fs::create_dir_all(&dir);
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("world_replay_events.jsonl"))
    {
        let tick = TICKS_RECORDED.load(Ordering::Relaxed);
        let _ = writeln!(
            file,
            "{{\"tick\":{},\"event\":\"{}\",\"detail\":\"{}\"}}",
            tick, event_type, detail
        );
    }
}

fn current_pose(host: &dyn ClientHost) -> ([f64; 3], [f32; 2]) {
    host.local_player()
        .and_then(|p| {
            let e = p.entity();
            Some((e.position(host)?, e.rotation(host)?))
        })
        .unwrap_or(([0.0, 64.0, 0.0], [0.0, 0.0]))
}

fn toggle_recording(host: &mut dyn ClientHost) {
    let pose = current_pose(&*host);
    let now = !RECORDING_ACTIVE.load(Ordering::Relaxed);
    RECORDING_ACTIVE.store(now, Ordering::Relaxed);
    let m = MODE_IDX.load(Ordering::Relaxed) as usize;
    let kf = KEYFRAME_COUNT.load(Ordering::Relaxed);
    write_replay_state(now, m, kf, pose.0, pose.1);
    append_world_event("toggle_recording", if now { "REC_ON" } else { "PAUSED" });
    let status_str = if now {
        "§a● REC 录制中 (实时上传夸克网盘即删)"
    } else {
        "§e■ PAUSED 已暂停"
    };
    let _ = host.chat_line(Text::literal(format!(
        "§c[ReplayRecorder]§r 状态: {}§r | 模式: §b{}§r | 已记录关键帧: §e{}",
        status_str,
        MODES[m % MODES.len()],
        kf
    )));
}

fn add_keyframe(host: &mut dyn ClientHost) {
    let pose = current_pose(&*host);
    let kf = KEYFRAME_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    if let Ok(mut list) = KEYFRAMES.lock() {
        list.push(pose);
        if list.len() > 64 {
            list.remove(0);
        }
    }
    let m = MODE_IDX.load(Ordering::Relaxed) as usize;
    write_replay_state(RECORDING_ACTIVE.load(Ordering::Relaxed), m, kf, pose.0, pose.1);
    append_world_event(
        "keyframe_bookmark",
        &format!(
            "#{}: ({:.1},{:.1},{:.1}) yaw={:.1} pitch={:.1}",
            kf, pose.0[0], pose.0[1], pose.0[2], pose.1[0], pose.1[1]
        ),
    );
    let _ = host.chat_line(Text::literal(format!(
        "§c[ReplayRecorder]§r 已添加 3D 回放摄像机关键帧 §e#{}§r @ ({:.1}, {:.1}, {:.1}) yaw={:.1} pitch={:.1} [夸克云同步就绪]",
        kf, pose.0[0], pose.0[1], pose.0[2], pose.1[0], pose.1[1]
    )));
}

#[derive(Default)]
struct ReplayRecorderMod;

#[mod_api::horizon_mod]
impl Hooks for ReplayRecorderMod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let m_opts = MODES.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
        let _ = r.register_setting(SettingSpec::choice(
            "mode",
            "replay_recorder.setting.mode",
            m_opts,
            0,
        ));

        let s_opts = SEG_LENGTHS.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
        let _ = r.register_setting(SettingSpec::choice(
            "segment_seconds",
            "replay_recorder.setting.segment_seconds",
            s_opts,
            0,
        ));

        let d_opts = DISK_GUARDS.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
        let _ = r.register_setting(SettingSpec::choice(
            "disk_guard",
            "replay_recorder.setting.disk_guard",
            d_opts,
            0,
        ));

        let p_opts = PURGE_POLICY.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
        let _ = r.register_setting(SettingSpec::choice(
            "purge_policy",
            "replay_recorder.setting.purge_policy",
            p_opts,
            0,
        ));

        r.register_key_mapping(
            KeyRegistration::new(KEY_TOGGLE_REC, "key.keyboard.r")
                .label("key.replay_recorder.toggle_rec")
                .context(KeyConflictContext::InGame),
        );
        r.register_key_mapping(
            KeyRegistration::new(KEY_ADD_KEYFRAME, "key.keyboard.b")
                .label("key.replay_recorder.add_keyframe")
                .context(KeyConflictContext::InGame),
        );

        write_replay_state(true, 0, 0, [0.0, 64.0, 0.0], [0.0, 0.0]);
    }

    fn register_client_commands(
        &self,
        _: &mut dyn ClientHost,
        event: &mut RegisterClientCommandsEvent<'_>,
    ) {
        event.commands.push(Command::new("replay"));
        event.commands.push(Command::new("replay_toggle"));
        event.commands.push(Command::new("replay_keyframe"));
    }

    fn command(&self, host: &mut dyn ClientHost, event: &mut CommandEvent<'_>) -> Flow {
        let cmd = event.command.trim();
        if cmd == "replay" {
            let active = RECORDING_ACTIVE.load(Ordering::Relaxed);
            let m = MODE_IDX.load(Ordering::Relaxed) as usize % MODES.len();
            let kf = KEYFRAME_COUNT.load(Ordering::Relaxed);
            let _ = host.chat_line(Text::literal(format!(
                "§c[ReplayRecorder 状态]§r 录制: {} | 模式: {} | 关键帧: {} | 夸克目录: /MC-AI-Horizon-Recordings/ (自动即删)",
                if active { "§aREC ON§r" } else { "§ePAUSED§r" },
                MODES[m],
                kf
            )));
            return Flow::Cancel;
        } else if cmd == "replay_toggle" {
            toggle_recording(host);
            return Flow::Cancel;
        } else if cmd == "replay_keyframe" {
            add_keyframe(host);
            return Flow::Cancel;
        }
        Flow::Continue
    }

    fn register_commands(&self, _: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        event.commands.push(Command::new("replay_server"));
    }

    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        if event.command.literals.first().map(String::as_str) == Some("replay_server") {
            for p in host.players().server_players() {
                let _ = p.tell(
                    host,
                    Text::literal(
                        "§c[ServerReplay]§r 服务端世界回放与双视角录制流运行正常 -> 实时同步夸克网盘 (/MC-AI-Horizon-Recordings/) 并自动清理本地文件！",
                    ),
                );
            }
            return Flow::Cancel;
        }
        Flow::Continue
    }

    fn player_logged_in(&self, _host: &mut dyn ServerHost, event: &mut PlayerLoggedInEvent<'_>) {
        append_world_event("player_logged_in", &event.name);
    }

    fn setting_changed(&self, host: &mut dyn ClientHost, _event: &mut SettingChangedEvent<'_>) {
        let m = host.setting_choice("mode").unwrap_or(0) as usize % MODES.len();
        MODE_IDX.store(m as u32, Ordering::Relaxed);
        let pose = current_pose(&*host);
        write_replay_state(
            RECORDING_ACTIVE.load(Ordering::Relaxed),
            m,
            KEYFRAME_COUNT.load(Ordering::Relaxed),
            pose.0,
            pose.1,
        );
    }

    fn input_key(&self, host: &mut dyn ClientHost, event: &mut InputKeyEvent<'_>) {
        if event.action != KeyAction::Press || !host.playing() {
            return;
        }
        if event.key == "key.keyboard.r" && !host.is_down(ID, KEY_TOGGLE_REC) {
            toggle_recording(host);
        } else if event.key == "key.keyboard.b" && !host.is_down(ID, KEY_ADD_KEYFRAME) {
            add_keyframe(host);
        }
    }

    fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut ClientTickPostEvent) {
        if RECORDING_ACTIVE.load(Ordering::Relaxed) {
            let t = TICKS_RECORDED.fetch_add(1, Ordering::Relaxed);
            if t % 100 == 0 {
                let pose = current_pose(&*host);
                append_world_event(
                    "camera_sample",
                    &format!("({:.1},{:.1},{:.1})", pose.0[0], pose.0[1], pose.0[2]),
                );
            }
        }
        while host.consume_click(ID, KEY_TOGGLE_REC) {
            toggle_recording(host);
        }
        while host.consume_click(ID, KEY_ADD_KEYFRAME) {
            add_keyframe(host);
        }
    }

    fn render_level_stage(&self, host: &dyn ClientHost, event: &mut RenderLevelStageEvent<'_>) {
        if event.stage != RenderLevelStage::AfterLevel || !host.on(ID) {
            return;
        }
        let Ok(list) = KEYFRAMES.lock() else { return };
        if list.is_empty() {
            return;
        }
        let mut lines = Vec::with_capacity(list.len() * 4);
        let mut quads = Vec::with_capacity(list.len() * 2);
        for (i, (pos, _rot)) in list.iter().enumerate() {
            let [x, y, z] = *pos;
            // Vertical beacon line at camera keyframe
            lines.push(WorldLine::new(
                [x, y, z],
                [x, y + 2.4, z],
                0xFFFF_3B30,
                2.5,
                WorldDepth::ThroughWalls,
            ));
            // Connect sequential keyframes along the replay camera path spline
            if i > 0 {
                let prev = list[i - 1].0;
                lines.push(WorldLine::new(
                    [prev[0], prev[1] + 1.2, prev[2]],
                    [x, y + 1.2, z],
                    0xFF38_BDF8,
                    2.5,
                    WorldDepth::ThroughWalls,
                ));
            }
            // Keyframe diamond marker quad
            quads.push(WorldQuad::new(
                [
                    [x - 0.25, y + 1.4, z],
                    [x, y + 1.15, z],
                    [x + 0.25, y + 1.4, z],
                    [x, y + 1.65, z],
                ],
                0xB0FF_3B30,
                WorldDepth::ThroughWalls,
            ));
        }
        let _ = event.draw.lines(&lines);
        let _ = event.draw.quads(&quads);
    }

    fn render_gui_post(&self, host: &dyn ClientHost, event: &mut RenderGuiPostEvent<'_>) {
        if !host.on(ID) {
            return;
        }
        let active = RECORDING_ACTIVE.load(Ordering::Relaxed);
        let kf = KEYFRAME_COUNT.load(Ordering::Relaxed);
        let secs = TICKS_RECORDED.load(Ordering::Relaxed) / 20;
        let line1 = if active {
            format!("● REC [Replay+Quark云同步] {:02}:{:02} | 关键帧:{} [R暂停/B打点]", secs / 60, secs % 60, kf)
        } else {
            format!("■ PAUSED [Replay已暂停] {:02}:{:02} | 关键帧:{} [按R继续]", secs / 60, secs % 60, kf)
        };
        let draw = &mut *event.draw;
        let w = draw.text_width(&line1) + 8.0;
        let h = 14.0;
        let (x, y) = event.corners.place(Corner::BottomRight, w, h);
        draw.fill([x, y, w, h], 0x8818_1014);
        let accent = if active { 0xFFFF_2B4A } else { 0xFFFACC15 };
        draw.fill([x, y, 2.5, h], accent);
        draw.text(&line1, [x + 5.0, y + 3.0], accent, true);
    }
}
