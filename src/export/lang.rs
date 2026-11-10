//! The language of a card, detected from its text, and what depends on it in
//! the PDF: typographic quotes and the titles of alert boxes. Detection is
//! automatic only (no setting); when `whatlang` is not confident, or the
//! language is not one of ours, the PDF is English.

use whatlang::Lang as Detected;

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
const GUILLEMETS: Quotes = Quotes {
    open2: "«",
    close2: "»",
    open1: "‘",
    close1: "’",
};

impl Lang {
    /// The language `whatlang` is confident about, if it is one of ours.
    pub fn detect(card: &Card) -> Option<Lang> {
        let info = whatlang::detect(&prose(card))?;
        if !info.is_reliable() {
            return None;
        }
        Some(match info.lang() {
            Detected::Eng => Lang::En,
            Detected::Deu => Lang::De,
            Detected::Fra => Lang::Fr,
            Detected::Spa => Lang::Es,
            Detected::Ita => Lang::It,
            Detected::Nld => Lang::Nl,
            Detected::Por => Lang::Pt,
            _ => return None,
        })
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
        }
    }

    pub fn quotes(self) -> &'static Quotes {
        match self {
            Lang::En | Lang::Nl | Lang::Pt => &EN,
            Lang::De => &DE,
            Lang::Fr => &FR,
            Lang::Es | Lang::It => &GUILLEMETS,
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
        ] {
            for kind in Alert::ALL {
                assert!(!lang.alert_title(kind).is_empty());
            }
        }
        assert_eq!(Lang::De.alert_title(Alert::Warning), "Warnung");
    }
}
