//! Java-YSM-style In-Game Model & Animation Quick Preview GUI Panel!
//! Opens via `O` key or `/ysm_gui` over the live world with automatic `ThirdPersonFront` 3D preview.

use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use mod_api::{
    CameraType, ClientHost, DrawContext, ScreenMouseButtonPressedPreEvent,
    ScreenMouseScrolledPreEvent, ScreenRenderPostEvent,
};

use crate::{
    sync_state_to_disk, ytree_engine, GESTURE_IDX, ID, MODEL_SPECS, NAMED_ANIM_IDX,
    NAMED_ANIM_LABEL, PERSONAS, PERSONA_IDX,
};

static ANIM_SCROLL: AtomicUsize = AtomicUsize::new(0);
static PREV_CAMERA: AtomicU32 = AtomicU32::new(0); // 0=First, 1=ThirdBack, 2=ThirdFront

const LEFT_W: f32 = 165.0;
const RIGHT_W: f32 = 185.0;
const MARGIN: f32 = 10.0;
const ROW_H: f32 = 18.0;

fn cam_to_u32(c: CameraType) -> u32 {
    match c {
        CameraType::FirstPerson => 0,
        CameraType::ThirdPersonBack => 1,
        CameraType::ThirdPersonFront => 2,
    }
}

fn u32_to_cam(v: u32) -> CameraType {
    match v {
        1 => CameraType::ThirdPersonBack,
        2 => CameraType::ThirdPersonFront,
        _ => CameraType::FirstPerson,
    }
}

pub fn open_ysm_gui(host: &mut dyn ClientHost) -> bool {
    if host.mod_screen() == Some(ID) {
        host.mod_screen_close(ID);
        return false;
    }
    PREV_CAMERA.store(cam_to_u32(host.camera_type()), Ordering::Relaxed);
    if host.mod_screen_open(ID) {
        host.set_camera_type(CameraType::ThirdPersonFront);
        ANIM_SCROLL.store(0, Ordering::Relaxed);
        true
    } else {
        false
    }
}

pub fn on_screen_closing(host: &dyn ClientHost) {
    if host.mod_screen() == Some(ID) {
        let prev = u32_to_cam(PREV_CAMERA.load(Ordering::Relaxed));
        host.set_camera_type(prev);
    }
}

fn hit(mx: f32, my: f32, r: [f32; 4]) -> bool {
    mx >= r[0] && mx <= r[0] + r[2] && my >= r[1] && my <= r[1] + r[3]
}

fn visible_rows(sh: f32) -> usize {
    let avail = (sh - 96.0).max(ROW_H * 4.0);
    (avail / ROW_H).floor() as usize
}

pub fn render_ysm_gui(host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
    if host.mod_screen() != Some(ID) {
        return;
    }
    let sw = event.screen.rect[2];
    let sh = event.screen.rect[3];
    let [mx, my] = event.screen.mouse;
    let draw: &mut dyn DrawContext = &mut *event.draw;

    let p_idx = PERSONA_IDX.load(Ordering::Relaxed) as usize % PERSONAS.len();
    let named_raw = NAMED_ANIM_IDX.load(Ordering::Relaxed);
    let ytrees = ytree_engine::get_ytree_models(&MODEL_SPECS);
    let anims: Vec<&str> = ytrees
        .get(p_idx % ytrees.len())
        .and_then(|m| m.as_ref())
        .map(|m| m.animation_names())
        .unwrap_or_default();

    // === LEFT PANEL: 3D Model Selector ===
    let lx = MARGIN;
    let ly = MARGIN;
    let lh = (sh - MARGIN * 2.0).max(160.0);
    draw.fill([lx, ly, LEFT_W, lh], 0xCC0E_121B);
    draw.fill([lx, ly, LEFT_W, 2.0], 0xFF38_BDF8);
    draw.text("YSM 3D 模型预览", [lx + 8.0, ly + 8.0], 0xFFFF_D700, true);
    draw.text("点击切换 (实时3D预览)", [lx + 8.0, ly + 20.0], 0xFF94_A3B8, false);

    let mut cy = ly + 36.0;
    for (i, spec) in PERSONAS.iter().enumerate() {
        let rect = [lx + 6.0, cy, LEFT_W - 12.0, ROW_H - 2.0];
        let active = i == p_idx;
        let hovered = hit(mx, my, rect);
        let bg = if active {
            0xDD1E_3A5F
        } else if hovered {
            0xBB25_2E42
        } else {
            0x8816_1B26
        };
        draw.fill(rect, bg);
        if active {
            draw.fill([rect[0], rect[1], 3.0, rect[3]], 0xFF00_0000 | spec.primary_rgb);
        }
        let color = if active { 0xFFFF_F59D } else { 0xFFE2_E8F0 };
        draw.text(spec.short_title, [rect[0] + 6.0, rect[1] + 4.0], color, active);
        cy += ROW_H;
    }

    // === RIGHT PANEL: Full YAN2 Animation Quick Preview ===
    let rx = (sw - RIGHT_W - MARGIN).max(LEFT_W + 40.0);
    let ry = MARGIN;
    let rh = lh;
    draw.fill([rx, ry, RIGHT_W, rh], 0xCC0E_121B);
    draw.fill([rx, ry, RIGHT_W, 2.0], 0xFFA8_55F7);
    let title = format!("全部动画预览 ({}个)", anims.len());
    draw.text(&title, [rx + 8.0, ry + 8.0], 0xFFFF_D700, true);
    draw.text("滚轮翻页 · 点击立即播放", [rx + 8.0, ry + 20.0], 0xFF94_A3B8, false);

    // Top action bar: [恢复自动] button
    let auto_rect = [rx + 6.0, ry + 34.0, RIGHT_W - 12.0, ROW_H - 2.0];
    let is_auto = named_raw == u32::MAX;
    let auto_bg = if is_auto {
        0xDD14_532D
    } else if hit(mx, my, auto_rect) {
        0xCC1E_40AF
    } else {
        0xAA1F_2937
    };
    draw.fill(auto_rect, auto_bg);
    draw.text(
        if is_auto { "● 自动状态机 (已启用)" } else { "↺ 恢复自动动作状态机" },
        [auto_rect[0] + 6.0, auto_rect[1] + 4.0],
        0xFF86_EFAC,
        true,
    );

    let max_vis = visible_rows(sh);
    let max_scroll = anims.len().saturating_sub(max_vis);
    let scroll = ANIM_SCROLL.load(Ordering::Relaxed).min(max_scroll);
    ANIM_SCROLL.store(scroll, Ordering::Relaxed);

    let list_y = ry + 34.0 + ROW_H + 4.0;
    for row in 0..max_vis {
        let idx = scroll + row;
        let Some(&name) = anims.get(idx) else { break };
        let rect = [rx + 6.0, list_y + row as f32 * ROW_H, RIGHT_W - 12.0, ROW_H - 2.0];
        let active = named_raw as usize == idx;
        let hovered = hit(mx, my, rect);
        let bg = if active {
            0xDD58_1C87
        } else if hovered {
            0xBB2D_3748
        } else {
            0x8816_1B26
        };
        draw.fill(rect, bg);
        if active {
            draw.fill([rect[0], rect[1], 3.0, rect[3]], 0xFFE8_79F9);
        }
        let label = format!("{}. {}", idx + 1, name);
        let color = if active { 0xFFFF_F59D } else { 0xFFE2_E8F0 };
        draw.text(&label, [rect[0] + 6.0, rect[1] + 4.0], color, active);
    }

    // Footer pagination info
    let foot_y = ry + rh - 18.0;
    let page_info = format!(
        "显示 {}-{} / {} (滚轮翻页)",
        if anims.is_empty() { 0 } else { scroll + 1 },
        (scroll + max_vis).min(anims.len()),
        anims.len()
    );
    draw.text(&page_info, [rx + 8.0, foot_y], 0xFF94_A3B8, false);

    // === BOTTOM CENTER BAR ===
    let bar_w = 210.0;
    let bar_x = (sw - bar_w) * 0.5;
    let bar_y = sh - 26.0;
    draw.fill([bar_x, bar_y, bar_w, 18.0], 0xBB0E_121B);
    let hint = "按 O 或 ESC 关闭预览 · 实时3D展示中";
    draw.text(hint, [bar_x + 12.0, bar_y + 5.0], 0xFFE0_F2FE, true);
}

pub fn handle_mouse_click(
    host: &dyn ClientHost,
    event: &mut ScreenMouseButtonPressedPreEvent<'_>,
) -> bool {
    if host.mod_screen() != Some(ID) || event.button != 0 {
        return false;
    }
    let sw = event.screen.rect[2];
    let sh = event.screen.rect[3];
    let [mx, my] = event.screen.mouse;

    // Check left model list
    let lx = MARGIN;
    let mut cy = MARGIN + 36.0;
    for i in 0..PERSONAS.len() {
        let rect = [lx + 6.0, cy, LEFT_W - 12.0, ROW_H - 2.0];
        if hit(mx, my, rect) {
            PERSONA_IDX.store(i as u32, Ordering::Relaxed);
            NAMED_ANIM_IDX.store(u32::MAX, Ordering::Relaxed);
            ANIM_SCROLL.store(0, Ordering::Relaxed);
            let g = GESTURE_IDX.load(Ordering::Relaxed) as usize;
            sync_state_to_disk(i, g);
            event.cancelled = true;
            return true;
        }
        cy += ROW_H;
    }

    // Check right animation panel
    let rx = (sw - RIGHT_W - MARGIN).max(LEFT_W + 40.0);
    let auto_rect = [rx + 6.0, MARGIN + 34.0, RIGHT_W - 12.0, ROW_H - 2.0];
    if hit(mx, my, auto_rect) {
        NAMED_ANIM_IDX.store(u32::MAX, Ordering::Relaxed);
        GESTURE_IDX.store(0, Ordering::Relaxed);
        event.cancelled = true;
        return true;
    }

    let p_idx = PERSONA_IDX.load(Ordering::Relaxed) as usize % PERSONAS.len();
    let ytrees = ytree_engine::get_ytree_models(&MODEL_SPECS);
    let anims: Vec<&str> = ytrees
        .get(p_idx % ytrees.len())
        .and_then(|m| m.as_ref())
        .map(|m| m.animation_names())
        .unwrap_or_default();

    let max_vis = visible_rows(sh);
    let scroll = ANIM_SCROLL.load(Ordering::Relaxed);
    let list_y = MARGIN + 34.0 + ROW_H + 4.0;
    for row in 0..max_vis {
        let idx = scroll + row;
        let Some(&name) = anims.get(idx) else { break };
        let rect = [rx + 6.0, list_y + row as f32 * ROW_H, RIGHT_W - 12.0, ROW_H - 2.0];
        if hit(mx, my, rect) {
            NAMED_ANIM_IDX.store(idx as u32, Ordering::Relaxed);
            if let Ok(mut s) = NAMED_ANIM_LABEL.get_or_init(|| std::sync::Mutex::new(String::new())).lock() {
                *s = name.to_string();
            }
            event.cancelled = true;
            return true;
        }
    }

    false
}

pub fn handle_mouse_scroll(
    host: &dyn ClientHost,
    event: &mut ScreenMouseScrolledPreEvent<'_>,
) -> bool {
    if host.mod_screen() != Some(ID) {
        return false;
    }
    let sh = event.screen.rect[3];
    let p_idx = PERSONA_IDX.load(Ordering::Relaxed) as usize % PERSONAS.len();
    let ytrees = ytree_engine::get_ytree_models(&MODEL_SPECS);
    let total = ytrees
        .get(p_idx % ytrees.len())
        .and_then(|m| m.as_ref())
        .map(|m| m.animation_names().len())
        .unwrap_or(0);
    let max_vis = visible_rows(sh);
    let max_scroll = total.saturating_sub(max_vis);
    let cur = ANIM_SCROLL.load(Ordering::Relaxed);
    let next = if event.scroll[1] < 0.0 {
        (cur + 3).min(max_scroll)
    } else if event.scroll[1] > 0.0 {
        cur.saturating_sub(3)
    } else {
        cur
    };
    ANIM_SCROLL.store(next, Ordering::Relaxed);
    event.cancelled = true;
    true
}
