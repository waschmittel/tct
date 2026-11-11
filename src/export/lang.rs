//! The language of a card, detected from its text, and what depends on it in
//! the PDF: typographic quotes and the titles of alert boxes. Detection is
//! automatic only (no setting); when there is too little text, or the
//! language is not one of ours, the PDF is English.

use whichlang::Lang as Detected;

use crate::alert::Alert;
use crate::model::card::Card;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Lang {
    #[default]
    En,
    De,
    Fr,
    Es,
    It,
    Nl,
    Pt,
    Sv,
    Tr,
}

/// Opening and closing double and single quotes.
pub struct Quotes {
    pub open2: &'static str,
    pub close2: &'static str,
    pub open1: &'static str,
    pub close1: &'static str,
}

const EN: Quotes = Quotes {
    open2: "“",
    close2: "”",
    open1: "‘",
    close1: "’",
};
const DE: Quotes = Quotes {
    open2: "„",
    close2: "“",
    open1: "‚",
    close1: "‘",
};
/// French sets guillemets off with narrow no-break spaces.
const FR: Quotes = Quotes {
    open2: "«\u{202f}",
    close2: "\u{202f}»",
    open1: "‘",
    close1: "’",
};
/// Swedish uses the same mark to open and to close.
const SV: Quotes = Quotes {
    open2: "”",
    close2: "”",
    open1: "’",
    close1: "’",
};
const GUILLEMETS: Quotes = Quotes {
    open2: "«",
    close2: "»",
    open1: "‘",
    close1: "’",
};

/// `whichlang` always answers, even for "Pizza" (Italian), so it is only
/// asked about text with at least this many words ...
const MIN_WORDS: usize = 4;
/// ... and this many letters; fewer say too little about the language.
const MIN_LETTERS: usize = 15;

/// Words of the text that tell about its language: runs of letters, without
/// URLs and the like (tokens holding `://` or `@`).
fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split_whitespace()
        .filter(|t| !t.contains("://") && !t.contains('@'))
        .flat_map(|t| t.split(|c: char| !c.is_alphabetic()))
        .filter(|w| !w.is_empty())
}

impl Lang {
    /// The language of the card, if it has enough text to tell.
    pub fn detect(card: &Card) -> Option<Lang> {
        let text = prose(card);
        let (count, letters) =
            words(&text).fold((0, 0), |(n, l), w| (n + 1, l + w.chars().count()));
        if count < MIN_WORDS || letters < MIN_LETTERS {
            return None;
        }
        let lang = match whichlang::detect_language(&text) {
            Detected::Eng => Lang::En,
            Detected::Deu => Lang::De,
            Detected::Fra => Lang::Fr,
            Detected::Spa => Lang::Es,
            Detected::Ita => Lang::It,
            Detected::Nld => Lang::Nl,
            Detected::Por => Lang::Pt,
            Detected::Swe => Lang::Sv,
            Detected::Tur => Lang::Tr,
            _ => return None,
        };
        (!lang.contradicted_by(&text)).then_some(lang)
    }

    /// Letters and words that do not occur in the language but in a relative
    /// `whichlang` does not know: Polish reads as Turkish, Danish and
    /// Norwegian as Swedish.
    fn contradicted_by(self, text: &str) -> bool {
        let (letters, words_) = match self {
            Lang::Tr => ("ąćęłńśźż", &["jest", "nie", "jak", "ale", "dla"][..]),
            Lang::Sv => (
                "æøþð",
                &["jeg", "ikke", "og", "også", "hvordan", "hvad", "hva"][..],
            ),
            _ => return false,
        };
        let text = text.to_lowercase();
        text.chars().any(|c| letters.contains(c)) || words(&text).any(|w| words_.contains(&w))
    }

    /// What the PDF of `card` is set in.
    pub fn of(card: &Card) -> Lang {
        Self::detect(card).unwrap_or_default()
    }

    /// Shown in the export dialog.
    pub fn describe(card: &Card) -> String {
        match Self::detect(card) {
            Some(lang) => lang.name().into(),
            None => "English (default)".into(),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::De => "Deutsch",
            Lang::Fr => "Français",
            Lang::Es => "Español",
            Lang::It => "Italiano",
            Lang::Nl => "Nederlands",
            Lang::Pt => "Português",
            Lang::Sv => "Svenska",
            Lang::Tr => "Türkçe",
        }
    }

    pub fn quotes(self) -> &'static Quotes {
        match self {
            Lang::En | Lang::Nl | Lang::Pt | Lang::Tr => &EN,
            Lang::Sv => &SV,
            Lang::De => &DE,
            Lang::Fr => &FR,
            Lang::Es | Lang::It => &GUILLEMETS,
        }
    }

    /// The alert title in capitals; Turkish has a dotted capital İ for i.
    pub fn alert_title_upper(self, kind: Alert) -> String {
        let title = self.alert_title(kind);
        match self {
            Lang::Tr => title.replace('i', "İ").to_uppercase(),
            _ => title.to_uppercase(),
        }
    }

    pub fn alert_title(self, kind: Alert) -> &'static str {
        use Alert::*;
        let [note, tip, important, caution, warning] = match self {
            Lang::En => ["Note", "Tip", "Important", "Caution", "Warning"],
            Lang::De => ["Hinweis", "Tipp", "Wichtig", "Vorsicht", "Warnung"],
            Lang::Fr => [
                "Remarque",
                "Astuce",
                "Important",
                "Attention",
                "Avertissement",
            ],
            Lang::Es => ["Nota", "Consejo", "Importante", "Precaución", "Advertencia"],
            Lang::It => [
                "Nota",
                "Suggerimento",
                "Importante",
                "Attenzione",
                "Avvertenza",
            ],
            Lang::Nl => ["Opmerking", "Tip", "Belangrijk", "Let op", "Waarschuwing"],
            Lang::Pt => ["Nota", "Dica", "Importante", "Cuidado", "Aviso"],
            Lang::Sv => ["Observera", "Tips", "Viktigt", "Se upp", "Varning"],
            Lang::Tr => ["Not", "İpucu", "Önemli", "Dikkat", "Uyarı"],
        };
        match kind {
            Note => note,
            Tip => tip,
            Important => important,
            Caution => caution,
            Warning => warning,
        }
    }
}

/// Title and description without code, which says nothing about the language.
fn prose(card: &Card) -> String {
    let mut text = card.title.clone();
    let mut fenced = false;
    for line in card.description.lines() {
        if line.starts_with("```") {
            fenced = !fenced;
        } else if !fenced {
            text.push('\n');
            let mut code = false;
            text.extend(line.split('`').filter(|_| {
                code = !code;
                code
            }));
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(title: &str, description: &str) -> Card {
        let mut card = Card::new(title.into());
        card.description = description.into();
        card
    }

    #[test]
    fn detects_supported_languages() {
        let cases = [
            (
                "The quick brown fox jumps over the lazy dog and then goes home. We will meet again tomorrow morning to discuss the plan for the next release of the project.",
                Lang::En,
            ),
            (
                "Der schnelle braune Fuchs springt über den faulen Hund und geht dann nach Hause. Wir treffen uns morgen früh wieder, um den Plan für die nächste Version des Projekts zu besprechen.",
                Lang::De,
            ),
            (
                "Le renard brun rapide saute par-dessus le chien paresseux puis rentre à la maison. Nous nous reverrons demain matin pour discuter du plan de la prochaine version du projet.",
                Lang::Fr,
            ),
            (
                "El rápido zorro marrón salta sobre el perro perezoso y luego vuelve a casa. Nos volveremos a reunir mañana por la mañana para hablar del plan de la próxima versión del proyecto.",
                Lang::Es,
            ),
            (
                "La rapida volpe marrone salta sopra il cane pigro e poi torna a casa. Ci rivedremo domani mattina per discutere il piano della prossima versione del progetto.",
                Lang::It,
            ),
            (
                "De snelle bruine vos springt over de luie hond en gaat daarna naar huis. Morgenochtend komen we weer bij elkaar om het plan voor de volgende versie van het project te bespreken.",
                Lang::Nl,
            ),
            (
                "A rápida raposa marrom pula sobre o cão preguiçoso e depois volta para casa. Vamos nos encontrar novamente amanhã de manhã para discutir o plano da próxima versão do projeto.",
                Lang::Pt,
            ),
            (
                "Den snabba bruna räven hoppar över den lata hunden och går sedan hem. Vi ses igen i morgon bitti för att diskutera planen för nästa version av projektet.",
                Lang::Sv,
            ),
            (
                "Hızlı kahverengi tilki tembel köpeğin üzerinden atlar ve sonra eve gider. Projenin bir sonraki sürümüyle ilgili planı görüşmek için yarın sabah tekrar buluşacağız.",
                Lang::Tr,
            ),
        ];
        for (text, lang) in cases {
            assert_eq!(Lang::detect(&card("", text)), Some(lang), "{text}");
        }
    }

    #[test]
    fn unsure_or_unsupported_is_english() {
        for text in [
            "",
            "ok",
            "42",
            "Zażółć gęślą jaźń, szybki brązowy lis przeskakuje nad leniwym psem.",
        ] {
            let card = card("", text);
            assert_eq!(Lang::detect(&card), None, "{text}");
            assert_eq!(Lang::of(&card), Lang::En);
            assert_eq!(Lang::describe(&card), "English (default)");
        }
    }

    #[test]
    fn code_does_not_count() {
        let german =
            "Der schnelle braune Fuchs springt über den faulen Hund und geht dann nach Hause.";
        let code =
            "```\nthe quick brown fox jumps over the lazy dog and goes home with the dog\n```\n";
        let card = card("", &format!("{code}{german} `the quick brown fox`"));
        assert_eq!(Lang::detect(&card), Some(Lang::De));
    }

    #[test]
    fn every_language_titles_every_alert() {
        for lang in [
            Lang::En,
            Lang::De,
            Lang::Fr,
            Lang::Es,
            Lang::It,
            Lang::Nl,
            Lang::Pt,
            Lang::Sv,
            Lang::Tr,
        ] {
            for kind in Alert::ALL {
                assert!(!lang.alert_title(kind).is_empty());
            }
        }
        assert_eq!(Lang::De.alert_title(Alert::Warning), "Warnung");
        assert_eq!(Lang::Tr.alert_title(Alert::Tip), "İpucu");
        assert_eq!(Lang::Tr.alert_title_upper(Alert::Important), "ÖNEMLİ");
        assert_eq!(Lang::Tr.alert_title_upper(Alert::Warning), "UYARI");
        assert_eq!(Lang::En.alert_title_upper(Alert::Tip), "TIP");
    }

    const BONJOUR: &str = "Bonjour ma cherie, je t'aime beaucoup.";

    #[test]
    fn short_sentences_are_detected() {
        let cases = [
            (BONJOUR, Lang::Fr),
            (
                "Bonjour tout le monde, comment allez-vous aujourd'hui ?",
                Lang::Fr,
            ),
            (
                "Je t'aime beaucoup, ma chérie. Où êtes-vous allés hier soir ?",
                Lang::Fr,
            ),
            (
                "Corriger le bug de connexion et mettre à jour la doc",
                Lang::Fr,
            ),
            ("Guten Morgen, wie geht es dir heute?", Lang::De),
            ("Login-Fehler beheben und Doku aktualisieren", Lang::De),
            ("Buenos días, ¿cómo estás hoy?", Lang::Es),
            ("Buongiorno, come stai oggi?", Lang::It),
            ("Goedemorgen, hoe gaat het vandaag met je?", Lang::Nl),
            ("Bom dia, como você está hoje?", Lang::Pt),
            (
                "Hej, hur mår du idag? Jag hoppas att allt är bra.",
                Lang::Sv,
            ),
            (
                "Fixa inloggningsfelet och uppdatera dokumentationen",
                Lang::Sv,
            ),
            (
                "Günaydın, bugün nasıl gidiyor? Umarım her şey yolundadır.",
                Lang::Tr,
            ),
            (
                "Kullanıcı girişi hatasını düzelt ve belgeleri güncelle",
                Lang::Tr,
            ),
            ("Good morning, how are you today?", Lang::En),
        ];
        for (text, lang) in cases {
            assert_eq!(Lang::detect(&card("", text)), Some(lang), "{text}");
        }
    }

    #[test]
    fn repetition_does_not_hurt() {
        let paragraphs = vec![BONJOUR; 12].join("\n\n");
        assert_eq!(Lang::detect(&card("", &paragraphs)), Some(Lang::Fr));
        let lines = vec![BONJOUR; 12].join("\n");
        assert_eq!(Lang::detect(&card("", &lines)), Some(Lang::Fr));
    }

    #[test]
    fn title_and_markup_count_as_prose() {
        let c = card(
            "Bonjour ma cherie",
            "**Je t'aime** beaucoup, _vraiment_.\n\n- oui\n- non",
        );
        assert_eq!(Lang::detect(&c), Some(Lang::Fr));
        assert_eq!(Lang::detect(&card(BONJOUR, "")), Some(Lang::Fr));
    }

    #[test]
    fn surrounding_whitespace_does_not_matter() {
        let c = card("", &format!("\n\n  {BONJOUR}  \n\n"));
        assert_eq!(Lang::detect(&c), Some(Lang::Fr));
    }

    #[test]
    fn code_around_short_text_does_not_flip_it() {
        let c = card(
            "",
            &format!("{BONJOUR}\n\n```\nfn main() {{ println!(\"hello world\"); }}\n```"),
        );
        assert_eq!(Lang::detect(&c), Some(Lang::Fr));
    }

    #[test]
    fn single_words_and_other_languages_stay_english() {
        for text in [
            "Pizza",
            "TODO",
            "Test",
            "Kafka",
            "https://example.com/a/b",
            "Привет, как дела сегодня?",
            "こんにちは、お元気ですか",
            "Dzień dobry, jak się dziś masz? Mam nadzieję, że wszystko dobrze.",
            "Hej, hvordan har du det i dag? Jeg håber, at alt er godt.", // Danish
            "Hei, hvordan har du det i dag? Jeg håper at alt er bra.",   // Norwegian
            "Jeg kjøpte brød og melk i butikken i går kveld.",
        ] {
            assert_eq!(Lang::detect(&card("", text)), None, "{text}");
        }
    }

    #[test]
    fn too_little_text_is_not_trusted() {
        // whichlang always answers (Italian for "Pizza"), so the amount of text gates it.
        assert_eq!(Lang::detect(&card("", "Merci beaucoup pour")), None); // 3 words
        assert_eq!(
            Lang::detect(&card("", "Merci beaucoup pour tout")),
            Some(Lang::Fr)
        );
        assert_eq!(Lang::detect(&card("", "Ja ja ja ja ja ja")), None); // 6 words, 12 letters
        assert_eq!(Lang::detect(&card("", "1 2 3 4 5 6 7 8 9 10 11 12")), None);
        assert_eq!(Lang::detect(&card("", "- - - - ... !!! ???")), None);
    }

    #[test]
    fn words_split_on_non_letters_and_skip_urls() {
        fn w(t: &str) -> Vec<&str> {
            words(t).collect()
        }
        assert_eq!(w("je t'aime-bien, 42x"), ["je", "t", "aime", "bien", "x"]);
        assert_eq!(
            w("see https://example.com/a and me@x.org now"),
            ["see", "and", "now"]
        );
        assert!(w("").is_empty());
    }

    #[test]
    fn urls_and_emails_do_not_make_text_long_enough() {
        let c = card(
            "",
            "https://example.com/a/b/c me@example.org https://x.y/z/w",
        );
        assert_eq!(Lang::detect(&c), None);
    }

    #[test]
    fn short_technical_titles_work_in_both_directions() {
        for (text, lang) in [
            ("Meeting notes for Q3 planning", Some(Lang::En)),
            ("Update README and CHANGELOG files", Some(Lang::En)),
            ("Kaufen Milch Brot Eier Butter", Some(Lang::De)),
            ("Zażółć gęślą jaźń szybki brązowy lis", None),
            ("Dzień dobry jak się masz dzisiaj", None),
        ] {
            assert_eq!(Lang::detect(&card("", text)), lang, "{text}");
        }
    }
}
