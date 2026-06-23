use eframe::egui::{self, ColorImage, TextureHandle};
use image::imageops::FilterType;
use rust_embed::RustEmbed;
use std::borrow::Cow;
use std::collections::HashMap;

pub const DEFAULT_CHARACTER: &str = "donna";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvatarState {
    Default,
    Idle(u8),
    Attention,
    Question,
    Thinking,
    Command,
}

#[derive(RustEmbed)]
#[folder = "assets/characters"]
struct CharacterAssets;

#[derive(Default)]
pub struct AvatarManager {
    textures: HashMap<String, TextureHandle>,
}

impl AvatarManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn resolve_character(configured: &str) -> String {
        if Self::character_exists(configured) {
            configured.to_owned()
        } else {
            DEFAULT_CHARACTER.to_owned()
        }
    }

    pub fn character_exists(name: &str) -> bool {
        CharacterAssets::get(&format!("{name}/default.png")).is_some()
    }

    pub fn asset_bytes(character: &str, state: AvatarState) -> Option<Cow<'static, [u8]>> {
        let character = Self::resolve_character(character);
        let path = format!("{character}/{}", state.file_name());

        CharacterAssets::get(&path)
            .map(|asset| asset.data)
            .or_else(|| {
                CharacterAssets::get(&format!("{character}/default.png")).map(|asset| asset.data)
            })
            .or_else(|| CharacterAssets::get("donna/default.png").map(|asset| asset.data))
    }

    pub fn texture_for(
        &mut self,
        ctx: &egui::Context,
        character: &str,
        state: AvatarState,
    ) -> Option<TextureHandle> {
        let character = Self::resolve_character(character);
        let key = format!("{character}:{}", state.file_name());

        if let Some(texture) = self.textures.get(&key) {
            return Some(texture.clone());
        }

        let bytes = Self::asset_bytes(&character, state)?;
        let max_texture_side = ctx.input(|input| input.max_texture_side);
        let image = decode_png(bytes.as_ref(), max_texture_side)?;
        let texture = ctx.load_texture(key.clone(), image, egui::TextureOptions::LINEAR);
        self.textures.insert(key, texture.clone());
        Some(texture)
    }
}

impl AvatarState {
    fn file_name(self) -> String {
        match self {
            AvatarState::Default => "default.png".to_owned(),
            AvatarState::Idle(frame) => format!("idle-{}.png", frame.clamp(1, 3)),
            AvatarState::Attention => "attention.png".to_owned(),
            AvatarState::Question => "question.png".to_owned(),
            AvatarState::Thinking => "thinking.png".to_owned(),
            AvatarState::Command => "command.png".to_owned(),
        }
    }
}

fn decode_png(bytes: &[u8], max_side: usize) -> Option<ColorImage> {
    let mut image = image::load_from_memory(bytes).ok()?.to_rgba8();
    let widest_side = image.width().max(image.height());

    if widest_side > max_side as u32 {
        let scale = max_side as f32 / widest_side as f32;
        let width = ((image.width() as f32 * scale).round() as u32).max(1);
        let height = ((image.height() as f32 * scale).round() as u32).max(1);
        image = image::imageops::resize(&image, width, height, FilterType::Lanczos3);
    }

    let size = [image.width() as usize, image.height() as usize];
    Some(ColorImage::from_rgba_unmultiplied(size, image.as_raw()))
}

#[cfg(test)]
mod tests {
    use super::{AvatarManager, AvatarState, DEFAULT_CHARACTER, decode_png};

    #[test]
    fn donna_character_is_embedded() {
        assert!(AvatarManager::character_exists(DEFAULT_CHARACTER));
        assert!(AvatarManager::asset_bytes(DEFAULT_CHARACTER, AvatarState::Default).is_some());
    }

    #[test]
    fn unknown_character_falls_back_to_donna() {
        let resolved = AvatarManager::resolve_character("unknown");

        assert_eq!(resolved, DEFAULT_CHARACTER);
        assert!(AvatarManager::asset_bytes("unknown", AvatarState::Thinking).is_some());
    }

    #[test]
    fn decoded_texture_respects_max_texture_side() {
        let bytes =
            AvatarManager::asset_bytes(DEFAULT_CHARACTER, AvatarState::Default).expect("asset");
        let image = decode_png(bytes.as_ref(), 512).expect("decoded image");

        assert!(image.width() <= 512);
        assert!(image.height() <= 512);
    }
}
