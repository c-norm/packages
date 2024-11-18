/// Reads the ncit codeSystem resource in fhir.tx.support.r4 and
/// a provided Codesystem. Compares their codes, adds the new ones 
/// to fhir.tx.support.r4 with the NCIT preferred term as the display
/// and adds synonyms to codes where the PQCMC preferred term differs.
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::fmt::Display;
use std::fs::File;
use std::io::{BufReader, BufWriter,Result};

const PATH: &str = "./packages/fhir.tx.support.r4/package/CodeSystem-nciThesaurus-fragment.json";
const OUT_PATH: &str = "output.json";
const DEFAULT_INPUT_PATH: &str = "new-codes.json";
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
/// only (de)serializes the properties of the original codesystem
struct CodeSystem {
    id: String,
    resource_type: String,
    url: String,
    name: String,
    title: String,
    status: String,
    experimental: bool,
    date: String,
    publisher: String,
    description: String,
    copyright: String,
    case_sensitive: bool,
    content: String,
    concept: Vec<Concept>,
}

struct Settings {
  suppress_info_level: bool
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Concept {
    code: String,
    display: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    designation: Option<Vec<Designation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    definition: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    concept: Option<Vec<Concept>>,
}

impl Display for Concept {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f,"{} \"{}\"",self.code,self.display)
    }
}

impl Concept {

  fn replace_display_with_new_term(self,new_display: String)->Self {
    // if the PQCMC preferred term is already the NCIT preferred term, don't 
    // do anything
    if self.display.to_lowercase() == new_display.to_ascii_lowercase() {
      Concept {
        display:new_display,
        // don't copy the definition
        definition:None,
        ..self
      }
    } else {
      // copy out the pqcmc preferred term
      let pqcmc_preferred_term = self.display.clone();
      println!("WARN:\tPQCMC term does not match NCIT preferred term:");
      println!("\t\tCode: {}",self.code);
      println!("\t\tPQCMC term: {}",pqcmc_preferred_term);
      println!("\t\tNCIT term:  {}",new_display);
      let mut new_concept = 
      Concept {
        display: new_display,
        // don't copy the definition
        definition:None,
        ..self
      };
      // add the pqcmc preferred term as a synonym
      new_concept.add_synonym(pqcmc_preferred_term);
      new_concept
    }
  }
  fn add_designation(&mut self, designation: Designation) {
    
    match self.designation.as_mut() {
      Some(v)=>v.push(designation),
      None => self.designation = Some(vec![designation])
    }
  }
  fn add_synonym(&mut self, synonym: String) {
    self.add_designation(Designation::synonym(synonym));
  }
}


#[derive(Serialize, Deserialize, Debug, Clone)]
struct Designation {
    #[serde(rename = "use")]
    #[serde(skip_serializing_if = "Option::is_none")]
    _use: Option<Use>,
    value: String,
}

impl Designation {
  fn synonym (synonym: String) -> Self{
    Designation { _use: Some(
      Use::synonym()
    ), value: synonym }
  }
}


#[derive(Serialize, Deserialize, Debug, Clone)]
struct Use {
    system: String,
    code: String,
}

impl Use {
  const SNOMED_SYSTEM: &str = "http://snomed.info/sct";
  const SYNONYM_SNOMED: &str = "900000000000013009";
  fn synonym () -> Self {
    Use { 
      system: Self::SNOMED_SYSTEM.to_string(), 
      code: Self::SYNONYM_SNOMED.to_string()
    }
  }
}

struct Statistics {
    already_exists: usize,
    wrong_display: usize,
    not_ncit_code: usize,
    new_code: usize,
}

impl Statistics {
    fn new() -> Self {
        Self {
            already_exists: 0,
            wrong_display: 0,
            new_code: 0,
            not_ncit_code: 0,
        }
    }
}

impl Display for Statistics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "STATISTICS:")?;
        writeln!(f, "pre-existing codes:\t{}", self.already_exists)?;
        writeln!(f, "wrong displays:\t\t{}", self.wrong_display)?;
        writeln!(f, "non-NCIT codes:\t\t{}", self.not_ncit_code)?;
        writeln!(f, "new codes:\t\t{}", self.new_code)
    }
}

impl CodeSystem {
  /// looks for a code in the vector of concepts. returns an option
  /// containing a borrowed, mutable concept if it finds one
  fn get_mutable_concept_by_code(&mut self, code: &String)->Option<&mut Concept> {
    self.concept.iter_mut().find(|x| &x.code == code)
  }
  /// push a concept into the codesystem. consumes the concept
  fn add_concept(&mut self, concept: Concept) {
    self.concept.push(concept)
  }
  /// push a concept if it's not there. If a concept already exists
  /// but the display is wrong and a synonym doesn't exist, add the
  /// new one as a synonym. updates statistics. consumes concept.
  fn check_and_add_concept(
    &mut self,
    concept: Concept,
    stats: &mut Statistics,
    settings: &Settings,
    thesaurus: &Thesaurus
  ) {
    if let Some(existing_concept) = self.get_mutable_concept_by_code(&concept.code) {
      stats.already_exists += 1;
      // if the codes are the same, but the display names are different, then
      // there is potentially something wrong. Don't care about case
      // check to see if the term already exists as a synonym 
      let does_not_exist_as_synonym = existing_concept.designation
        .clone().unwrap_or(Vec::new())
        .iter().find(|f|f.value.to_lowercase() == concept.display.to_lowercase()).is_none();
      if existing_concept.display.to_lowercase() != concept.display.to_lowercase() && does_not_exist_as_synonym{
        println!("WARN:\tMismatched displays for code '{}':", concept.code);
        println!(
            "\told: '{}'\r\n\tnew: '{}'",
            existing_concept.display, concept.display
        );
        existing_concept.add_synonym(concept.display);
        stats.wrong_display += 1;
      } else {
        if !settings.suppress_info_level {
          println!(
            "INFO:\tcode '{}' already present with correct display",
            concept.code
          )
        }
      }
    } else {
      // only add codes that can be found in the thesaurus
      if let Some(row) = thesaurus.get(&concept.code) {
        self.add_concept(concept.replace_display_with_new_term(row.get_ncit_preferred_term()));
        stats.new_code += 1;
      }
      else {
        println!("ERR:\tnon-NCIT code: {}", concept);
        stats.not_ncit_code += 1;
      }

    }
  }
  /// read a Codesystem resource from disk
  fn from_file(path: &str) -> Result<Self> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let json = serde_json::from_reader(reader)?;
    Ok(json)
  }
  /// write a Codesystem resource to disk
  fn to_file(self,path: &str) -> Result<()> {
    let file = File::create(path)?;
    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, &self)?;
    Ok(())
  }
}
#[derive(Debug)]
#[allow(dead_code)]
struct ThesaurusRow {
  code: String,
  iri: String,
  parent: Vec<String>,
  // first synonym is always preferred term
  synonyms: Vec<String>, // pipe delimited, handled in deserializer
  definition: String,
  display_name: Option<String>, // may be empty
  concept_status: Option<String>, // may be empty
  semantic_type: String,
  concept_in_subset: Vec<String> // pipe delimited, may be empty
}
impl ThesaurusRow {
  /// every term has at least one synonym. the first term is always the NCIT
  /// preferred term per the README file on NCIT's ftp server
  fn get_ncit_preferred_term(&self)->String{
    self.synonyms[0].clone()
  }
}

impl<'de> Deserialize<'de> for ThesaurusRow {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de> {
        #[derive(Deserialize)]
        struct RawRow {
          code: String,
          iri: String,
          parent: String,
          synonyms: String, // pipe delimited
          definition: String,
          display_name: Option<String>,
          concept_status: Option<String>,
          semantic_type: String,
          concept_in_subset: String // pipe delimited
        }
        fn split(s:&String)->Vec<String> {
          s.clone().split('|')
          .map(|x| x.to_string())
          .collect()
        }
        fn raw_row_to_thesaurus_row(raw:RawRow)->ThesaurusRow {
          ThesaurusRow {
            code: raw.code,
            iri: raw.iri,
            parent: split(&raw.parent),
            synonyms: split(&raw.synonyms),
            definition: raw.definition,
            display_name: raw.display_name,
            concept_status: raw.concept_status,
            semantic_type: raw.semantic_type,
            concept_in_subset: split(&raw.concept_in_subset),
          }
        }
        let raw_row: RawRow = Deserialize::deserialize(deserializer)?;
        Ok(raw_row_to_thesaurus_row(raw_row))
    }
}

type Thesaurus = HashMap<String,ThesaurusRow>;

fn thesaurus_from_file(path: String)->Result<Thesaurus> {
  let file = File::open(path)?;
  let reader = BufReader::new(file);
  let mut map: Thesaurus =HashMap::new();
  let mut reed = csv::ReaderBuilder::new().has_headers(false).delimiter(b'\t').from_reader(reader);
  for result in reed.deserialize() {
    let record: ThesaurusRow = result?;
    map.insert(record.code.clone(), record);
  }
  Ok(map)
}
fn main() {
    let thesaurus = 
      thesaurus_from_file(
        std::env::var("THESAURUS")
        .unwrap_or("Thesaurus.txt".to_string())
      )
      .expect("couldn't read thesaurus");

    let settings = Settings {
      suppress_info_level: true
    };
    let mut system =
      CodeSystem::from_file(PATH)
      .expect("something went wrong reading old codes");
    let mut stats = Statistics::new();
    let new_codes =
      CodeSystem::from_file(&*std::env::var("NEW_CODES")
      .unwrap_or(DEFAULT_INPUT_PATH.to_string()))
      .expect("something went wrong reading new codes");
    for concept in new_codes.concept {
      system.check_and_add_concept(concept, &mut stats, &settings, &thesaurus);
    }
    system.to_file(OUT_PATH).expect("couldn't write to disk");
    println!("{}", stats);
}
