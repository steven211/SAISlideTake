use crate::error::{CoreError, CoreResult};
use crate::types::{DeckParserProfile, ParserSlidePreview, ParserTestResult};
use quick_xml::{events::Event, Reader};
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    io::{Cursor, Read},
    path::{Component, Path, PathBuf},
};
use zip::ZipArchive;

const MAX_HTML_BYTES: u64 = 20 * 1024 * 1024;
const MAX_PPTX_BYTES: u64 = 512 * 1024 * 1024;
const MAX_XML_BYTES: u64 = 16 * 1024 * 1024;
const MAX_SLIDES: usize = 10_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeckSlide {
    pub slide_id: String,
    pub slide_index: usize,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParsedDeck {
    pub title: String,
    pub source_path: String,
    pub format: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge_path: Option<String>,
    pub slides: Vec<DeckSlide>,
}

pub trait DeckParser {
    fn parse(&self, path: &Path) -> CoreResult<ParsedDeck>;
}

pub struct HtmlDeckParser;
pub struct PptxDeckParser;

pub fn parse_deck(path: &Path) -> CoreResult<ParsedDeck> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    match extension.to_ascii_lowercase().as_str() {
        "html" | "htm" => HtmlDeckParser.parse(path),
        "pptx" => PptxDeckParser.parse(path),
        _ => Err(CoreError::Invalid(
            "僅支援 HTML、HTM 與 PPTX 簡報".to_string(),
        )),
    }
}

impl DeckParser for HtmlDeckParser {
    fn parse(&self, path: &Path) -> CoreResult<ParsedDeck> {
        let bytes = bounded_file(path, MAX_HTML_BYTES)?;
        let source = String::from_utf8_lossy(&bytes);
        let document = Html::parse_document(&source);
        let reveal_selector = selector(".slides > section")?;
        let marp_selector = selector(
            ".marp-slide, section[data-marpit-pagination], section[data-marpit-pagination-total]",
        )?;
        let generic_selector = selector(".slide, .chapter")?;
        let mut elements = Vec::new();

        for root in document.select(&reveal_selector) {
            append_reveal_slides(root, &mut elements);
        }
        if elements.is_empty() {
            elements.extend(document.select(&marp_selector));
        }
        if elements.is_empty() {
            for element in document.select(&generic_selector) {
                let nested = element
                    .ancestors()
                    .filter_map(ElementRef::wrap)
                    .any(|parent| {
                        parent
                            .value()
                            .classes()
                            .any(|class| matches!(class, "slide" | "chapter"))
                    });
                if !nested {
                    elements.push(element);
                }
            }
        }
        if elements.is_empty() {
            return Err(CoreError::Invalid(
                "找不到簡報頁面；HTML 需包含 Reveal section、.marp-slide、.slide 或 .chapter"
                    .to_string(),
            ));
        }
        if elements.len() > MAX_SLIDES {
            return Err(CoreError::Invalid("簡報頁數超過 10000 頁上限".to_string()));
        }
        let file_title = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("簡報");
        let title = document_title(&document).unwrap_or_else(|| file_title.to_string());
        let mut slides = Vec::with_capacity(elements.len());
        let mut used_ids = std::collections::HashSet::new();
        for (index, element) in elements.into_iter().enumerate() {
            let heading = element
                .select(&selector("h1, h2, .slide-title")?)
                .next()
                .map(|node| clean_text(&node.text().collect::<String>()));
            let title = element
                .value()
                .attr("data-title")
                .map(str::to_owned)
                .or_else(|| heading.filter(|value| !value.is_empty()))
                .unwrap_or_else(|| format!("第 {} 頁", index + 1));
            let candidate = element
                .value()
                .attr("data-slide-id")
                .or_else(|| element.value().attr("id"))
                .map(str::to_owned)
                .unwrap_or_else(|| format!("slide_{:02}_{}", index + 1, slug(&title)));
            let slide_id = unique_slide_id(&candidate, index, &mut used_ids);
            slides.push(DeckSlide {
                slide_id,
                slide_index: index,
                title,
                notes: None,
            });
        }
        Ok(ParsedDeck {
            title,
            source_path: path.to_string_lossy().into_owned(),
            format: "html".to_string(),
            bridge_path: None,
            slides,
        })
    }
}

pub fn parse_html_with_profile(path: &Path, profile: &DeckParserProfile) -> CoreResult<ParsedDeck> {
    validate_profile(profile)?;
    let bytes = bounded_file(path, MAX_HTML_BYTES)?;
    let source = String::from_utf8_lossy(&bytes);
    let document = Html::parse_document(&source);
    let mut elements = profile_slide_elements(&document, profile)?;
    let mut fallback_titles = Vec::new();
    if elements.is_empty() {
        match profile.fallback_strategy.as_str() {
            "heading_h1" => elements = document.select(&selector("h1")?).collect(),
            "heading_h2" => elements = document.select(&selector("h2")?).collect(),
            "hr_split" => {
                let page_count = document.select(&selector("hr")?).count().saturating_add(1);
                let headings = document
                    .select(&selector("h1, h2")?)
                    .map(|heading| clean_text(&heading.text().collect::<String>()))
                    .filter(|title| !title.is_empty())
                    .collect::<Vec<_>>();
                let total = page_count.max(headings.len()).min(MAX_SLIDES);
                fallback_titles = (0..total)
                    .map(|index| {
                        headings
                            .get(index)
                            .cloned()
                            .unwrap_or_else(|| format!("第 {} 頁", index + 1))
                    })
                    .collect();
            }
            "manual" => {}
            _ => unreachable!("profile is validated before parsing"),
        }
    }

    if !fallback_titles.is_empty() {
        return parsed_deck_from_fallback(path, &document, fallback_titles);
    }
    if elements.is_empty() {
        return Err(CoreError::Invalid(format!(
            "解析器「{}」找不到符合 {} 的投影片節點",
            profile.engine_name, profile.slide_selector
        )));
    }
    if elements.len() > MAX_SLIDES {
        return Err(CoreError::Invalid("簡報頁數超過 10000 頁上限".to_string()));
    }

    let file_title = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("簡報");
    let deck_title = document_title(&document).unwrap_or_else(|| file_title.to_string());
    let title_selector = if profile.title_selector.trim().is_empty() {
        selector("h1, h2, .slide-title")?
    } else {
        selector(&profile.title_selector)?
    };
    let notes_selector = if profile.notes_selector.trim().is_empty() {
        None
    } else {
        Some(selector(&profile.notes_selector)?)
    };
    let mut used_ids = std::collections::HashSet::new();
    let slides = elements
        .into_iter()
        .enumerate()
        .map(|(index, element)| {
            let heading = element
                .select(&title_selector)
                .next()
                .map(|node| {
                    node.value()
                        .attr("data-title")
                        .map(str::to_owned)
                        .unwrap_or_else(|| clean_text(&node.text().collect::<String>()))
                })
                .filter(|value| !value.is_empty())
                .or_else(|| {
                    matches!(element.value().name(), "h1" | "h2" | "h3")
                        .then(|| clean_text(&element.text().collect::<String>()))
                        .filter(|value| !value.is_empty())
                });
            let title = element
                .value()
                .attr("data-title")
                .map(str::to_owned)
                .or(heading)
                .unwrap_or_else(|| format!("第 {} 頁", index + 1));
            let candidate = element
                .value()
                .attr("data-slide-id")
                .or_else(|| element.value().attr("id"))
                .map(str::to_owned)
                .unwrap_or_else(|| format!("slide_{:02}_{}", index + 1, slug(&title)));
            let notes = notes_selector.as_ref().and_then(|notes| {
                element
                    .select(notes)
                    .next()
                    .map(|node| clean_text(&node.text().collect::<String>()))
                    .filter(|value| !value.is_empty())
            });
            DeckSlide {
                slide_id: unique_slide_id(&candidate, index, &mut used_ids),
                slide_index: index,
                title,
                notes,
            }
        })
        .collect();
    Ok(ParsedDeck {
        title: deck_title,
        source_path: path.to_string_lossy().into_owned(),
        format: "html".to_string(),
        bridge_path: None,
        slides,
    })
}

pub fn test_html_parser_profile(
    path: &Path,
    profile: &DeckParserProfile,
) -> CoreResult<ParserTestResult> {
    let parsed = parse_html_with_profile(path, profile)?;
    let bytes = bounded_file(path, MAX_HTML_BYTES)?;
    let source = String::from_utf8_lossy(&bytes);
    let document = Html::parse_document(&source);
    let active_tokens = profile
        .active_class
        .split(',')
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    let active_selectors = active_tokens
        .iter()
        .filter(|token| token.starts_with('.') || token.starts_with('#') || token.starts_with('['))
        .map(|token| selector(token))
        .collect::<CoreResult<Vec<_>>>()?;
    let elements = profile_slide_elements(&document, profile)?;
    let slides = parsed
        .slides
        .into_iter()
        .map(|slide| {
            let element = elements.get(slide.slide_index).copied();
            let active = element.is_some_and(|element| {
                active_tokens.iter().any(|token| {
                    !token.starts_with('.')
                        && !token.starts_with('#')
                        && !token.starts_with('[')
                        && element.value().classes().any(|class| class == *token)
                }) || active_selectors
                    .iter()
                    .any(|active| active.matches(&element))
            });
            ParserSlidePreview {
                slide_index: slide.slide_index,
                title: slide.title,
                notes: slide.notes,
                active,
            }
        })
        .collect::<Vec<_>>();
    Ok(ParserTestResult {
        total_slides: slides.len(),
        slides,
        error: None,
    })
}

pub fn profile_bridge_slide_count(path: &Path, profile: &DeckParserProfile) -> CoreResult<usize> {
    validate_profile(profile)?;
    let bytes = bounded_file(path, MAX_HTML_BYTES)?;
    let source = String::from_utf8_lossy(&bytes);
    let document = Html::parse_document(&source);
    Ok(profile_slide_elements(&document, profile)?.len())
}

fn profile_slide_elements<'a>(
    document: &'a Html,
    profile: &DeckParserProfile,
) -> CoreResult<Vec<ElementRef<'a>>> {
    let slide_selector = selector(&profile.slide_selector)?;
    let mut elements = document.select(&slide_selector).collect::<Vec<_>>();
    // Reveal's outer section is a group when it contains vertical slides; only its leaves are pages.
    if profile.engine_name.to_ascii_lowercase().contains("reveal") {
        let mut leaves = Vec::new();
        for element in elements {
            append_reveal_slides(element, &mut leaves);
        }
        elements = leaves;
    } else {
        elements.retain(|element| {
            !element
                .ancestors()
                .filter_map(ElementRef::wrap)
                .any(|ancestor| slide_selector.matches(&ancestor))
        });
    }
    if !elements.is_empty() {
        return Ok(elements);
    }
    match profile.fallback_strategy.as_str() {
        "heading_h1" => Ok(document.select(&selector("h1")?).collect()),
        "heading_h2" => Ok(document.select(&selector("h2")?).collect()),
        "hr_split" => Ok(document.select(&selector("h1, h2")?).collect()),
        "manual" => Ok(Vec::new()),
        _ => unreachable!("profile is validated before parsing"),
    }
}

fn validate_profile(profile: &DeckParserProfile) -> CoreResult<()> {
    if profile.engine_name.trim().is_empty() || profile.engine_name.len() > 80 {
        return Err(CoreError::Invalid(
            "解析器名稱必須為 1 至 80 個字元".to_string(),
        ));
    }
    if profile.slide_selector.trim().is_empty() || profile.slide_selector.len() > 512 {
        return Err(CoreError::Invalid(
            "投影片 CSS 選擇器必須為 1 至 512 個字元".to_string(),
        ));
    }
    selector(&profile.slide_selector)?;
    for value in [&profile.title_selector, &profile.notes_selector] {
        if value.len() > 512 {
            return Err(CoreError::Invalid(
                "標題／備忘稿選擇器不可超過 512 個字元".to_string(),
            ));
        }
        if !value.trim().is_empty() {
            selector(value)?;
        }
    }
    if profile.active_class.len() > 512 {
        return Err(CoreError::Invalid(
            "啟用狀態選擇器不可超過 512 個字元".to_string(),
        ));
    }
    for token in profile
        .active_class
        .split(',')
        .map(str::trim)
        .filter(|item| item.starts_with('.') || item.starts_with('#') || item.starts_with('['))
    {
        selector(token)?;
    }
    if !matches!(
        profile.fallback_strategy.as_str(),
        "heading_h1" | "heading_h2" | "hr_split" | "manual"
    ) {
        return Err(CoreError::Invalid("不支援的解析備援策略".to_string()));
    }
    Ok(())
}

fn parsed_deck_from_fallback(
    path: &Path,
    document: &Html,
    titles: Vec<String>,
) -> CoreResult<ParsedDeck> {
    if titles.len() > MAX_SLIDES {
        return Err(CoreError::Invalid("簡報頁數超過 10000 頁上限".to_string()));
    }
    let file_title = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("簡報");
    let title = document_title(document).unwrap_or_else(|| file_title.to_string());
    let mut used_ids = std::collections::HashSet::new();
    let slides = titles
        .into_iter()
        .enumerate()
        .map(|(index, title)| DeckSlide {
            slide_id: unique_slide_id(
                &format!("slide_{:02}_{}", index + 1, slug(&title)),
                index,
                &mut used_ids,
            ),
            slide_index: index,
            title,
            notes: None,
        })
        .collect();
    Ok(ParsedDeck {
        title,
        source_path: path.to_string_lossy().into_owned(),
        format: "html".to_string(),
        bridge_path: None,
        slides,
    })
}

impl DeckParser for PptxDeckParser {
    fn parse(&self, path: &Path) -> CoreResult<ParsedDeck> {
        let bytes = bounded_file(path, MAX_PPTX_BYTES)?;
        let mut archive = ZipArchive::new(Cursor::new(bytes))
            .map_err(|error| CoreError::Invalid(format!("PPTX ZIP 格式錯誤：{error}")))?;
        let presentation = read_zip_entry(&mut archive, "ppt/presentation.xml")?;
        let relationships = read_zip_entry(&mut archive, "ppt/_rels/presentation.xml.rels")?;
        let slide_rel_ids = parse_presentation_order(&presentation)?;
        if slide_rel_ids.is_empty() || slide_rel_ids.len() > MAX_SLIDES {
            return Err(CoreError::Invalid(
                "PPTX 頁數為零或超過 10000 頁上限".to_string(),
            ));
        }
        let rel_targets = parse_relationships(&relationships)?;
        let title = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("簡報")
            .to_string();
        let mut slides = Vec::with_capacity(slide_rel_ids.len());
        let mut used_ids = std::collections::HashSet::new();
        for (index, rel_id) in slide_rel_ids.iter().enumerate() {
            let target = rel_targets
                .get(rel_id)
                .ok_or_else(|| CoreError::Invalid(format!("PPTX 缺少投影片關聯 {rel_id}")))?;
            let slide_path = resolve_package_target("ppt", target)?;
            let slide_xml = read_zip_entry(&mut archive, &slide_path)?;
            let slide_text = parse_shape_text(&slide_xml)?;
            let slide_title = slide_text
                .iter()
                .find(|shape| shape.kind == "title" && !shape.text.is_empty())
                .or_else(|| slide_text.iter().find(|shape| !shape.text.is_empty()))
                .map(|shape| shape.text.clone())
                .unwrap_or_else(|| format!("第 {} 頁", index + 1));
            let slide_id = unique_slide_id(
                &format!("pptx_{:02}_{}", index + 1, slug(&slide_title)),
                index,
                &mut used_ids,
            );
            let notes = slide_relationship_path(&slide_path)
                .and_then(|rels_path| read_zip_entry(&mut archive, &rels_path).ok())
                .and_then(|rels| parse_relationships(&rels).ok())
                .and_then(|rels| {
                    rels.values()
                        .find(|target| {
                            target.ends_with("notesSlide") || target.contains("notesSlide")
                        })
                        .cloned()
                })
                .and_then(|target| resolve_package_target("ppt/slides", &target).ok())
                .and_then(|notes_path| read_zip_entry(&mut archive, &notes_path).ok())
                .and_then(|xml| parse_shape_text(&xml).ok())
                .map(|shapes| {
                    shapes
                        .into_iter()
                        .filter(|shape| shape.kind == "body" || shape.kind.is_empty())
                        .map(|shape| shape.text)
                        .filter(|text| !text.is_empty())
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .filter(|notes| !notes.is_empty());
            slides.push(DeckSlide {
                slide_id,
                slide_index: index,
                title: slide_title,
                notes,
            });
        }
        Ok(ParsedDeck {
            title,
            source_path: path.to_string_lossy().into_owned(),
            format: "pptx".to_string(),
            bridge_path: None,
            slides,
        })
    }
}

fn append_reveal_slides<'a>(element: ElementRef<'a>, output: &mut Vec<ElementRef<'a>>) {
    let nested: Vec<_> = element
        .children()
        .filter_map(ElementRef::wrap)
        .filter(|child| child.value().name() == "section")
        .collect();
    if nested.is_empty() {
        output.push(element);
    } else {
        for child in nested {
            append_reveal_slides(child, output);
        }
    }
}

fn selector(source: &str) -> CoreResult<Selector> {
    Selector::parse(source)
        .map_err(|error| CoreError::Invalid(format!("HTML selector 無效：{error}")))
}

fn document_title(document: &Html) -> Option<String> {
    let title = document
        .select(&selector("title").ok()?)
        .next()?
        .text()
        .collect::<String>();
    let title = clean_text(&title);
    (!title.is_empty()).then_some(title)
}

fn clean_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn slug(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_whitespace() || matches!(character, '/' | '\\') {
                '_'
            } else {
                character
            }
        })
        .take(72)
        .collect()
}

fn unique_slide_id(
    candidate: &str,
    index: usize,
    used: &mut std::collections::HashSet<String>,
) -> String {
    let safe = candidate
        .chars()
        .filter(|character| {
            !character.is_control() && !matches!(character, '/' | '\\' | ':' | '\0')
        })
        .take(140)
        .collect::<String>();
    let base = if safe.trim().is_empty() {
        format!("slide_{:02}", index + 1)
    } else {
        safe
    };
    let mut id = base.clone();
    let mut suffix = 2;
    while !used.insert(id.clone()) {
        id = format!("{base}_{suffix}");
        suffix += 1;
    }
    id
}

fn bounded_file(path: &Path, max_bytes: u64) -> CoreResult<Vec<u8>> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(CoreError::Invalid("簡報來源必須是檔案".to_string()));
    }
    if metadata.len() > max_bytes {
        return Err(CoreError::Invalid("簡報檔案超過大小上限".to_string()));
    }
    Ok(fs::read(path)?)
}

fn read_zip_entry(archive: &mut ZipArchive<Cursor<Vec<u8>>>, name: &str) -> CoreResult<String> {
    let entry = archive
        .by_name(name)
        .map_err(|error| CoreError::Invalid(format!("PPTX 缺少 {name}：{error}")))?;
    if entry.size() > MAX_XML_BYTES {
        return Err(CoreError::Invalid(format!("PPTX XML 過大：{name}")));
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry.take(MAX_XML_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_XML_BYTES {
        return Err(CoreError::Invalid(format!("PPTX XML 超過 16MB：{name}")));
    }
    String::from_utf8(bytes)
        .map_err(|error| CoreError::Invalid(format!("PPTX XML 編碼錯誤：{error}")))
}

fn parse_presentation_order(xml: &str) -> CoreResult<Vec<String>> {
    let mut reader = Reader::from_str(xml);
    let mut ids = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) | Ok(Event::Empty(event))
                if local_name(event.name().as_ref()) == b"sldId" =>
            {
                for attribute in event.attributes().with_checks(false) {
                    let attribute = attribute.map_err(|error| {
                        CoreError::Invalid(format!("PPTX presentation XML 錯誤：{error}"))
                    })?;
                    if attribute.key.as_ref().ends_with(b":id") {
                        ids.push(
                            attribute
                                .unescape_value()
                                .map_err(|error| CoreError::Invalid(error.to_string()))?
                                .into_owned(),
                        );
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => {
                return Err(CoreError::Invalid(format!(
                    "PPTX presentation XML 錯誤：{error}"
                )))
            }
            _ => {}
        }
    }
    Ok(ids)
}

fn parse_relationships(xml: &str) -> CoreResult<HashMap<String, String>> {
    let mut reader = Reader::from_str(xml);
    let mut relationships = HashMap::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) | Ok(Event::Empty(event))
                if local_name(event.name().as_ref()) == b"Relationship" =>
            {
                let mut id = None;
                let mut target = None;
                for attribute in event.attributes().with_checks(false) {
                    let attribute = attribute.map_err(|error| {
                        CoreError::Invalid(format!("PPTX rels XML 錯誤：{error}"))
                    })?;
                    match local_name(attribute.key.as_ref()) {
                        b"Id" => {
                            id = Some(
                                attribute
                                    .unescape_value()
                                    .map_err(|error| CoreError::Invalid(error.to_string()))?
                                    .into_owned(),
                            )
                        }
                        b"Target" => {
                            target = Some(
                                attribute
                                    .unescape_value()
                                    .map_err(|error| CoreError::Invalid(error.to_string()))?
                                    .into_owned(),
                            )
                        }
                        _ => {}
                    }
                }
                if let (Some(id), Some(target)) = (id, target) {
                    relationships.insert(id, target);
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(CoreError::Invalid(format!("PPTX rels XML 錯誤：{error}"))),
            _ => {}
        }
    }
    Ok(relationships)
}

#[derive(Default)]
struct ShapeText {
    kind: String,
    text: String,
}

fn parse_shape_text(xml: &str) -> CoreResult<Vec<ShapeText>> {
    let mut reader = Reader::from_str(xml);
    let mut shapes = Vec::new();
    let mut current: Option<ShapeText> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) if local_name(event.name().as_ref()) == b"sp" => {
                current = Some(ShapeText::default())
            }
            Ok(Event::Empty(event)) if local_name(event.name().as_ref()) == b"ph" => {
                if let Some(shape) = current.as_mut() {
                    shape.kind = attr_value(&event, b"type").unwrap_or_default();
                }
            }
            Ok(Event::Start(event)) if local_name(event.name().as_ref()) == b"ph" => {
                if let Some(shape) = current.as_mut() {
                    shape.kind = attr_value(&event, b"type").unwrap_or_default();
                }
            }
            Ok(Event::Text(event)) => {
                if let Some(shape) = current.as_mut() {
                    let raw = String::from_utf8_lossy(event.as_ref());
                    let decoded = quick_xml::escape::unescape(&raw)
                        .map_err(|error| CoreError::Invalid(error.to_string()))?;
                    if !shape.text.is_empty() && !decoded.trim().is_empty() {
                        shape.text.push(' ');
                    }
                    shape.text.push_str(decoded.as_ref());
                }
            }
            Ok(Event::End(event)) if local_name(event.name().as_ref()) == b"sp" => {
                if let Some(mut shape) = current.take() {
                    shape.text = clean_text(&shape.text);
                    if !shape.text.is_empty() {
                        shapes.push(shape);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(CoreError::Invalid(format!("PPTX slide XML 錯誤：{error}"))),
            _ => {}
        }
    }
    Ok(shapes)
}

fn attr_value(event: &quick_xml::events::BytesStart<'_>, wanted: &[u8]) -> Option<String> {
    event
        .attributes()
        .with_checks(false)
        .flatten()
        .find(|attribute| local_name(attribute.key.as_ref()) == wanted)
        .and_then(|attribute| {
            attribute
                .unescape_value()
                .ok()
                .map(|value| value.into_owned())
        })
}

fn local_name(name: &[u8]) -> &[u8] {
    name.rsplit(|byte| *byte == b':').next().unwrap_or(name)
}

fn slide_relationship_path(slide_path: &str) -> Option<String> {
    let file = Path::new(slide_path).file_name()?.to_str()?;
    Some(format!("ppt/slides/_rels/{file}.rels"))
}

fn resolve_package_target(base: &str, target: &str) -> CoreResult<String> {
    let mut path = if target.starts_with('/') {
        PathBuf::new()
    } else {
        PathBuf::from(base)
    };
    let target_path = target.trim_start_matches('/').replace('\\', "/");
    for component in Path::new(&target_path).components() {
        match component {
            Component::Normal(value) => path.push(value),
            Component::ParentDir => {
                if !path.pop() {
                    return Err(CoreError::Invalid("PPTX relationship 路徑越界".to_string()));
                }
            }
            Component::CurDir => {}
            _ => {
                return Err(CoreError::Invalid(
                    "PPTX relationship 路徑不合法".to_string(),
                ))
            }
        }
    }
    Ok(path.to_string_lossy().replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn parses_nested_reveal_slides_in_presentation_order() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("deck.html");
        fs::write(&path, r#"<title>My Deck</title><div class="slides"><section><h1>Group</h1><section id="one"><h1>One</h1></section><section data-title="Two"><h2>Ignored</h2></section></section><section id="three"><h1>Three</h1></section></div>"#).unwrap();
        let deck = HtmlDeckParser.parse(&path).unwrap();
        assert_eq!(deck.title, "My Deck");
        assert_eq!(
            deck.slides
                .iter()
                .map(|slide| slide.slide_id.as_str())
                .collect::<Vec<_>>(),
            ["one", "slide_02_Two", "three"]
        );
        assert_eq!(deck.slides[1].title, "Two");
    }

    #[test]
    fn parses_marp_and_generic_slide_containers() {
        let directory = TempDir::new().unwrap();
        let marp = directory.path().join("marp.html");
        fs::write(&marp, r#"<div class="marp-slide" data-title="Intro"></div><div class="marp-slide" id="body"><h1>Body</h1></div>"#).unwrap();
        assert_eq!(HtmlDeckParser.parse(&marp).unwrap().slides.len(), 2);
        let generic = directory.path().join("generic.html");
        fs::write(&generic, r#"<article class="chapter" id="a"><h1>A</h1><div class="slide" id="nested">Nested</div></article><article class="slide" id="b">B</article>"#).unwrap();
        let deck = HtmlDeckParser.parse(&generic).unwrap();
        assert_eq!(deck.slides.len(), 2);
        assert_eq!(deck.slides[0].slide_id, "a");
    }

    #[test]
    fn parses_custom_profile_titles_notes_and_current_slide_state() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("custom.html");
        fs::write(
            &path,
            r#"<article class="page current" id="intro"><header>Custom intro</header><aside class="speaker-note">Say hello</aside></article><article class="page"><header>Custom body</header></article>"#,
        )
        .unwrap();
        let profile = DeckParserProfile {
            engine_name: "Custom pages".to_string(),
            slide_selector: ".page".to_string(),
            active_class: "current".to_string(),
            title_selector: "header".to_string(),
            notes_selector: ".speaker-note".to_string(),
            fallback_strategy: "manual".to_string(),
        };

        let preview = test_html_parser_profile(&path, &profile).unwrap();
        assert_eq!(preview.total_slides, 2);
        assert_eq!(preview.slides[0].title, "Custom intro");
        assert_eq!(preview.slides[0].notes.as_deref(), Some("Say hello"));
        assert!(preview.slides[0].active);
        assert!(!preview.slides[1].active);
    }

    #[test]
    fn profile_fallbacks_can_preview_heading_and_hr_split_pages() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("fallback.html");
        fs::write(&path, "<h1>First</h1><p>Text</p><hr><h1>Second</h1>").unwrap();
        let base = DeckParserProfile {
            engine_name: "Custom".to_string(),
            slide_selector: ".missing-slide".to_string(),
            active_class: String::new(),
            title_selector: "h1".to_string(),
            notes_selector: String::new(),
            fallback_strategy: "heading_h1".to_string(),
        };
        let headings = parse_html_with_profile(&path, &base).unwrap();
        assert_eq!(headings.slides.len(), 2);
        assert_eq!(headings.slides[1].title, "Second");

        let split = DeckParserProfile {
            fallback_strategy: "hr_split".to_string(),
            ..base
        };
        let preview = test_html_parser_profile(&path, &split).unwrap();
        assert_eq!(preview.total_slides, 2);
        assert_eq!(preview.slides[0].title, "First");
        assert_eq!(preview.slides[1].title, "Second");
    }

    #[test]
    fn profile_rejects_invalid_css_and_fallback_values() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("invalid.html");
        fs::write(&path, "<div></div>").unwrap();
        let profile = DeckParserProfile {
            engine_name: "Broken".to_string(),
            slide_selector: "[".to_string(),
            active_class: String::new(),
            title_selector: String::new(),
            notes_selector: String::new(),
            fallback_strategy: "unknown".to_string(),
        };
        assert!(parse_html_with_profile(&path, &profile).is_err());
    }

    #[test]
    fn resolves_relative_openxml_targets_without_extraction() {
        assert_eq!(
            resolve_package_target("ppt", "slides/slide7.xml").unwrap(),
            "ppt/slides/slide7.xml"
        );
        assert_eq!(
            resolve_package_target("ppt/slides", "../notesSlides/notesSlide2.xml").unwrap(),
            "ppt/notesSlides/notesSlide2.xml"
        );
        assert!(resolve_package_target("ppt", "../../../outside").is_err());
    }

    #[test]
    fn parses_pptx_slide_order_titles_and_speaker_notes() {
        use std::io::Write;
        use zip::{write::SimpleFileOptions, ZipWriter};
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("ordered.pptx");
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let files = [
            (
                "ppt/presentation.xml",
                r#"<p:presentation xmlns:p="p" xmlns:r="r"><p:sldIdLst><p:sldId id="512" r:id="rId9"/><p:sldId id="256" r:id="rId3"/></p:sldIdLst></p:presentation>"#,
            ),
            (
                "ppt/_rels/presentation.xml.rels",
                r#"<Relationships><Relationship Id="rId9" Target="slides/slide7.xml"/><Relationship Id="rId3" Target="slides/slide2.xml"/></Relationships>"#,
            ),
            (
                "ppt/slides/slide7.xml",
                r#"<p:sld xmlns:p="p" xmlns:a="a"><p:sp><p:nvSpPr><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr><p:txBody><a:p><a:r><a:t>Opening</a:t></a:r></a:p></p:txBody></p:sp></p:sld>"#,
            ),
            (
                "ppt/slides/_rels/slide7.xml.rels",
                r#"<Relationships><Relationship Id="rId4" Target="../notesSlides/notesSlide4.xml" Type="notesSlide"/></Relationships>"#,
            ),
            (
                "ppt/notesSlides/notesSlide4.xml",
                r#"<p:notes xmlns:p="p" xmlns:a="a"><p:sp><p:nvSpPr><p:nvPr><p:ph type="body"/></p:nvPr></p:nvSpPr><p:txBody><a:p><a:r><a:t>Welcome the audience</a:t></a:r></a:p></p:txBody></p:sp></p:notes>"#,
            ),
            (
                "ppt/slides/slide2.xml",
                r#"<p:sld xmlns:p="p" xmlns:a="a"><p:sp><p:nvSpPr><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr><p:txBody><a:p><a:r><a:t>Second</a:t></a:r></a:p></p:txBody></p:sp></p:sld>"#,
            ),
        ];
        for (name, contents) in files {
            writer
                .start_file(name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(contents.as_bytes()).unwrap();
        }
        fs::write(&path, writer.finish().unwrap().into_inner()).unwrap();
        let deck = PptxDeckParser.parse(&path).unwrap();
        assert_eq!(
            deck.slides
                .iter()
                .map(|slide| slide.title.as_str())
                .collect::<Vec<_>>(),
            ["Opening", "Second"]
        );
        assert_eq!(
            deck.slides[0].notes.as_deref(),
            Some("Welcome the audience")
        );
    }
}
