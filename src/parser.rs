use std::collections::HashMap;
use std::rc::Rc;

use pest::error::LineColLocation;
use pest::{iterators::Pairs, Parser};
use pest_derive::Parser;

#[derive(Default, Debug)]
pub struct DB {
    pub entries: Vec<Entry>,
    pub categories: HashMap<String, Vec<String>>,
    pub questions: HashMap<String, String>,
    pub changes: HashMap<String, String>,
    pub tips: HashMap<String, String>,
}

#[derive(Default, Debug)]
pub struct Entry {
    pub value: String,
    pub category: String,
    pub categories: Vec<(String, String)>,
}

pub enum ParserError {
    Parse(Rc<String>, (usize, usize)),
}

#[derive(Parser)]
#[grammar = "syn.pest"]
struct LangParser;

pub fn parse_db_from_file(contents: &str) -> Result<DB, ParserError> {
    let file = LangParser::parse(Rule::file, contents)
        .map_err(|err| {
            let pos = match err.line_col {
                LineColLocation::Pos((x, y)) => (x, y),
                LineColLocation::Span((start_x, start_y), _) => (start_x, start_y),
            };
            ParserError::Parse(Rc::new(err.to_string()), pos)
        })?
        .next()
        .unwrap();

    let mut db = DB::new();
    for data in file.into_inner() {
        match data.as_rule() {
            Rule::entry => parse_entry(&mut data.into_inner(), &mut db),
            Rule::advice => parse_advice(&mut data.into_inner(), &mut db.questions),
            Rule::change => parse_change(&mut data.into_inner(), &mut db.changes),
            Rule::tip => parse_tip(&mut data.into_inner(), &mut db.tips),
            Rule::EOI => break,
            _ => unreachable!(),
        }
    }

    Ok(db)
}

impl DB {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            categories: HashMap::new(),
            questions: HashMap::new(),
            changes: HashMap::new(),
            tips: HashMap::new(),
        }
    }

    fn add_category(&mut self, category: &str, value: &str) {
        if let Some(values) = self.categories.get_mut(category) {
            if !values.iter().any(|x| *x == value) {
                values.push(value.to_string());
            }
            return;
        }

        self.categories
            .insert(category.to_string(), vec![value.to_string()]);
    }

    pub fn find_value(
        &self,
        target_category: Option<&String>,
        query: Vec<(&String, &String)>,
    ) -> Option<String> {
        let mut sub_categories_to_match = Vec::new();

        if let Some(target_cat) = target_category {
            for entry in &self.entries {
                if &entry.category == target_cat {
                    for (cat, val) in &entry.categories {
                        if query
                            .iter()
                            .any(|&(q_cat, q_val)| q_cat == cat && q_val == val)
                        {
                            sub_categories_to_match.push((cat.clone(), val.clone()));
                        }
                    }
                }
            }
        }

        self.entries
            .iter()
            .find(|entry| {
                sub_categories_to_match.iter().all(|(sub_cat, sub_val)| {
                    entry
                        .categories
                        .iter()
                        .any(|(cat, val)| cat == sub_cat && val == sub_val)
                })
            })
            .map(|entry| entry.value.clone())
    }
}

fn parse_entry(entry: &mut Pairs<'_, Rule>, db: &mut DB) {
    let _number = entry.next().unwrap().as_str().parse::<i32>().unwrap();

    let mut pairs = Vec::<(String, String)>::new();
    entry.next().unwrap().into_inner().for_each(|x| {
        let mut pair = x.into_inner();
        let category = pair.next().unwrap().as_str().to_string();
        let value = pair.next().unwrap().as_str().to_string();

        db.add_category(&category, &value);
        pairs.push((category, value));
    });

    let mut pair = entry.next().unwrap().into_inner();
    let category = pair.next().unwrap().as_str().to_string();
    let value = pair.next().unwrap().as_str().to_string();

    db.add_category(&category, &value);

    db.entries.push(Entry {
        value,
        category,
        categories: pairs,
    });
}

fn parse_advice(advice: &mut Pairs<'_, Rule>, questions: &mut HashMap<String, String>) {
    let category = advice.next().unwrap().as_str().to_string();
    let question = advice.next().unwrap().as_str().to_string();

    questions.insert(category, question);
}

fn parse_change(change: &mut Pairs<'_, Rule>, changes: &mut HashMap<String, String>) {
    let category = change.next().unwrap().as_str().to_string();
    let text = change.next().unwrap().as_str().to_string();

    changes.insert(category, text);
}

fn parse_tip(change: &mut Pairs<'_, Rule>, tips: &mut HashMap<String, String>) {
    let category = change.next().unwrap().as_str().to_string();
    let text = change.next().unwrap().as_str().to_string();

    tips.insert(category, text);
}
