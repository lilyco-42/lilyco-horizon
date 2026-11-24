//! Fast, zero-unwrap Bedrock Molang expression parser & evaluator.
//! Supports `math.*` (angles in DEGREES per Bedrock spec), `q.*`/`query.*`,
//! `v.*`/`variable.*`, `t.*`/`temp.*`, `?:`, comparisons, `&&`/`||`/`!`.

use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub struct MolangContext {
    pub anim_time: f32,
    pub life_time: f32,
    pub modified_distance_moved: f32,
    pub modified_move_speed: f32,
    pub ground_speed: f32,
    pub head_x_rotation: f32,
    pub head_y_rotation: f32,
    pub is_sneaking: bool,
    pub is_on_ground: bool,
    pub is_sprinting: bool,
    pub is_swimming: bool,
    pub hurt_time: f32,
    pub vars: HashMap<String, f32>,
}

impl MolangContext {
    pub fn query(&self, name: &str) -> f32 {
        match name {
            "anim_time" => self.anim_time,
            "life_time" => self.life_time,
            "modified_distance_moved" => self.modified_distance_moved,
            "modified_move_speed" => self.modified_move_speed,
            "ground_speed" => self.ground_speed,
            "head_x_rotation" => self.head_x_rotation,
            "head_y_rotation" => self.head_y_rotation,
            "is_sneaking" | "is_crouching" => if self.is_sneaking { 1.0 } else { 0.0 },
            "is_on_ground" => if self.is_on_ground { 1.0 } else { 0.0 },
            "is_sprinting" => if self.is_sprinting { 1.0 } else { 0.0 },
            "is_swimming" | "is_in_water" => if self.is_swimming { 1.0 } else { 0.0 },
            "hurt_time" => self.hurt_time,
            "actor_count" | "is_alive" => 1.0,
            _ => 0.0,
        }
    }
}

#[derive(Clone, Debug)]
enum Tok {
    Num(f32),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
    Comma,
    Question,
    Colon,
    Semi,
    Eq,
    EqEq,
    NotEq,
    Lt,
    Le,
    Gt,
    Ge,
    AndAnd,
    OrOr,
    Bang,
}

fn tokenize(input: &str) -> Vec<Tok> {
    let bytes = input.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if b.is_ascii_digit() || (b == b'.' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit()) {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
                i += 1;
                if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
                    i += 1;
                }
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let num = input[start..i].parse::<f32>().unwrap_or(0.0);
            out.push(Tok::Num(num));
            continue;
        }
        if b.is_ascii_alphabetic() || b == b'_' {
            let start = i;
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'.')
            {
                i += 1;
            }
            out.push(Tok::Ident(input[start..i].to_ascii_lowercase()));
            continue;
        }
        let next = if i + 1 < bytes.len() { bytes[i + 1] } else { 0 };
        match (b, next) {
            (b'=', b'=') => { out.push(Tok::EqEq); i += 2; }
            (b'!', b'=') => { out.push(Tok::NotEq); i += 2; }
            (b'<', b'=') => { out.push(Tok::Le); i += 2; }
            (b'>', b'=') => { out.push(Tok::Ge); i += 2; }
            (b'&', b'&') => { out.push(Tok::AndAnd); i += 2; }
            (b'|', b'|') => { out.push(Tok::OrOr); i += 2; }
            (b'+', _) => { out.push(Tok::Plus); i += 1; }
            (b'-', _) => { out.push(Tok::Minus); i += 1; }
            (b'*', _) => { out.push(Tok::Star); i += 1; }
            (b'/', _) => { out.push(Tok::Slash); i += 1; }
            (b'(', _) => { out.push(Tok::LParen); i += 1; }
            (b')', _) => { out.push(Tok::RParen); i += 1; }
            (b',', _) => { out.push(Tok::Comma); i += 1; }
            (b'?', _) => { out.push(Tok::Question); i += 1; }
            (b':', _) => { out.push(Tok::Colon); i += 1; }
            (b';', _) => { out.push(Tok::Semi); i += 1; }
            (b'=', _) => { out.push(Tok::Eq); i += 1; }
            (b'<', _) => { out.push(Tok::Lt); i += 1; }
            (b'>', _) => { out.push(Tok::Gt); i += 1; }
            (b'!', _) => { out.push(Tok::Bang); i += 1; }
            _ => { i += 1; }
        }
    }
    out
}

pub fn eval_molang(expr: &str, ctx: &MolangContext) -> f32 {
    if let Ok(n) = expr.trim().parse::<f32>() {
        return n;
    }
    let toks = tokenize(expr);
    if toks.is_empty() {
        return 0.0;
    }
    let mut locals: HashMap<String, f32> = HashMap::new();
    // Models like Gemini_柴郡猫娘 write `1+v.roaming.size` (additive delta, default 0.0),
    // while models like DeepSeek酱 write `v.roaming.Size` (direct scale, default 1.0).
    let compact: String = expr.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.to_ascii_lowercase().contains("1+v.roaming.size") {
        locals.insert("v.roaming.size".to_string(), 0.0);
    }
    let mut parser = Parser {
        toks: &toks,
        pos: 0,
        ctx,
        locals: &mut locals,
    };
    let mut last = 0.0;
    while parser.pos < parser.toks.len() {
        if matches!(parser.peek(), Some(Tok::Semi)) {
            parser.pos += 1;
            continue;
        }
        if matches!(parser.peek(), Some(Tok::Ident(s)) if s == "return") {
            parser.pos += 1;
            return parser.parse_ternary();
        }
        last = parser.parse_assign();
        if matches!(parser.peek(), Some(Tok::Semi)) {
            parser.pos += 1;
        } else {
            break;
        }
    }
    if last.is_finite() { last } else { 0.0 }
}

struct Parser<'a> {
    toks: &'a [Tok],
    pos: usize,
    ctx: &'a MolangContext,
    locals: &'a mut HashMap<String, f32>,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn parse_assign(&mut self) -> f32 {
        if let (Some(Tok::Ident(name)), Some(Tok::Eq)) =
            (self.toks.get(self.pos), self.toks.get(self.pos + 1))
        {
            let var_name = name.clone();
            self.pos += 2;
            let val = self.parse_ternary();
            self.locals.insert(var_name, val);
            return val;
        }
        self.parse_ternary()
    }

    fn parse_ternary(&mut self) -> f32 {
        let cond = self.parse_or();
        if matches!(self.peek(), Some(Tok::Question)) {
            self.pos += 1;
            let then_v = self.parse_ternary();
            let else_v = if matches!(self.peek(), Some(Tok::Colon)) {
                self.pos += 1;
                self.parse_ternary()
            } else {
                0.0
            };
            if cond != 0.0 { then_v } else { else_v }
        } else {
            cond
        }
    }

    fn parse_or(&mut self) -> f32 {
        let mut left = self.parse_and();
        while matches!(self.peek(), Some(Tok::OrOr)) {
            self.pos += 1;
            let right = self.parse_and();
            left = if left != 0.0 || right != 0.0 { 1.0 } else { 0.0 };
        }
        left
    }

    fn parse_and(&mut self) -> f32 {
        let mut left = self.parse_cmp();
        while matches!(self.peek(), Some(Tok::AndAnd)) {
            self.pos += 1;
            let right = self.parse_cmp();
            left = if left != 0.0 && right != 0.0 { 1.0 } else { 0.0 };
        }
        left
    }

    fn parse_cmp(&mut self) -> f32 {
        let mut left = self.parse_add();
        loop {
            let op = match self.peek() {
                Some(Tok::EqEq) => 0,
                Some(Tok::NotEq) => 1,
                Some(Tok::Lt) => 2,
                Some(Tok::Le) => 3,
                Some(Tok::Gt) => 4,
                Some(Tok::Ge) => 5,
                _ => break,
            };
            self.pos += 1;
            let right = self.parse_add();
            let res = match op {
                0 => (left - right).abs() < 1e-6,
                1 => (left - right).abs() >= 1e-6,
                2 => left < right,
                3 => left <= right,
                4 => left > right,
                _ => left >= right,
            };
            left = if res { 1.0 } else { 0.0 };
        }
        left
    }

    fn parse_add(&mut self) -> f32 {
        let mut left = self.parse_mul();
        loop {
            match self.peek() {
                Some(Tok::Plus) => { self.pos += 1; left += self.parse_mul(); }
                Some(Tok::Minus) => { self.pos += 1; left -= self.parse_mul(); }
                _ => break,
            }
        }
        left
    }

    fn parse_mul(&mut self) -> f32 {
        let mut left = self.parse_unary();
        loop {
            match self.peek() {
                Some(Tok::Star) => { self.pos += 1; left *= self.parse_unary(); }
                Some(Tok::Slash) => {
                    self.pos += 1;
                    let r = self.parse_unary();
                    left = if r.abs() > 1e-9 { left / r } else { 0.0 };
                }
                _ => break,
            }
        }
        left
    }

    fn parse_unary(&mut self) -> f32 {
        match self.peek() {
            Some(Tok::Minus) => { self.pos += 1; -self.parse_unary() }
            Some(Tok::Plus) => { self.pos += 1; self.parse_unary() }
            Some(Tok::Bang) => { self.pos += 1; if self.parse_unary() == 0.0 { 1.0 } else { 0.0 } }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> f32 {
        match self.peek().cloned() {
            Some(Tok::Num(n)) => { self.pos += 1; n }
            Some(Tok::LParen) => {
                self.pos += 1;
                let v = self.parse_ternary();
                if matches!(self.peek(), Some(Tok::RParen)) {
                    self.pos += 1;
                }
                v
            }
            Some(Tok::Ident(name)) => {
                self.pos += 1;
                if matches!(self.peek(), Some(Tok::LParen)) {
                    self.pos += 1;
                    let mut args = Vec::new();
                    if !matches!(self.peek(), Some(Tok::RParen)) {
                        loop {
                            args.push(self.parse_ternary());
                            if matches!(self.peek(), Some(Tok::Comma)) {
                                self.pos += 1;
                            } else {
                                break;
                            }
                        }
                    }
                    if matches!(self.peek(), Some(Tok::RParen)) {
                        self.pos += 1;
                    }
                    return eval_fn(&name, &args);
                }
                resolve_ident(&name, self.ctx, self.locals)
            }
            _ => {
                self.pos += 1;
                0.0
            }
        }
    }
}

fn resolve_ident(name: &str, ctx: &MolangContext, locals: &HashMap<String, f32>) -> f32 {
    if let Some(&v) = locals.get(name) {
        return v;
    }
    if name == "math.pi" {
        return std::f32::consts::PI;
    }
    if let Some(q) = name.strip_prefix("q.").or_else(|| name.strip_prefix("query.")) {
        return ctx.query(q);
    }
    if let Some(v) = name.strip_prefix("v.").or_else(|| name.strip_prefix("variable.")) {
        if let Some(&val) = ctx.vars.get(v) {
            return val;
        }
        // Official YSM `v.roaming.Size` / `v.roaming.HeadSize` / `*scale*` default to 1.0!
        if v.contains("size") || v.contains("scale") {
            return 1.0;
        }
        return 0.0;
    }
    0.0
}

fn eval_fn(name: &str, args: &[f32]) -> f32 {
    let a0 = args.first().copied().unwrap_or(0.0);
    let a1 = args.get(1).copied().unwrap_or(0.0);
    let a2 = args.get(2).copied().unwrap_or(0.0);
    match name {
        // Bedrock Molang trig functions take DEGREES!
        "math.sin" => a0.to_radians().sin(),
        "math.cos" => a0.to_radians().cos(),
        "math.abs" => a0.abs(),
        "math.clamp" => a0.clamp(a1.min(a2), a1.max(a2)),
        "math.min" => a0.min(a1),
        "math.max" => a0.max(a1),
        "math.lerp" => a0 + (a1 - a0) * a2,
        "math.floor" => a0.floor(),
        "math.ceil" => a0.ceil(),
        "math.round" => a0.round(),
        "math.sqrt" => if a0 >= 0.0 { a0.sqrt() } else { 0.0 },
        "math.pow" => a0.powf(a1),
        "math.mod" => if a1.abs() > 1e-9 { a0 % a1 } else { 0.0 },
        "math.hermite_blend" => {
            let t = a0.clamp(0.0, 1.0);
            3.0 * t * t - 2.0 * t * t * t
        }
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_real_bedrock_molang_expressions() {
        let ctx = MolangContext {
            anim_time: 1.0,
            modified_distance_moved: 10.0,
            modified_move_speed: 0.8,
            head_y_rotation: 25.0,
            is_sneaking: true,
            ..Default::default()
        };
        assert!((eval_molang("math.sin(90)", &ctx) - 1.0).abs() < 1e-5);
        assert!((eval_molang("math.cos(180)", &ctx) - (-1.0)).abs() < 1e-5);
        assert!((eval_molang("q.is_sneaking ? 24.0 : 0.0", &ctx) - 24.0).abs() < 1e-5);
        assert!(
            (eval_molang("t.x = q.head_y_rotation * 0.5; return t.x + 2.5;", &ctx) - 15.0).abs()
                < 1e-5
        );
    }
}
