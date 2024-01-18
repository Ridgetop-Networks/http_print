 /*  config.toml needed in root directory containing these fields:    *
 *      url = "https://ridgetop.sonar.software/api/graphql"          *
*     api_key = """KEY GOES HERE"""                                 */

use config::{Config, ConfigError};         //config builder
use reqwest::{Client, header::HeaderMap};  //http requests
use serde::Deserialize;                    //json
use csv::Writer;                           //csv and file writing


//TODO clean up error handling
//TODO write tests and test error handling
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    //load the config
    let config = load_config()?;
    //let data = config.data;
    //this query gets the total number of contacts in the database by settting records per page = 1 and returning the number of pages. tis what it is :(
    let enum_query = r#"
    {
        "query": "{\r\n  phone_numbers(paginator: {page: 1, records_per_page: 1}) {\r\n    page_info {\r\n total_pages }\r\n}}"
    }
    "#;
    //serialize into valid json
    let enum_query_json: serde_json::Value = serde_json::from_str(&enum_query)?;
    //build the client object
    let client = reqwest::Client::builder().build()?;
    //define our headers which mainly come from config
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Content-Type", "application/json".parse()?);
    headers.insert("Authorization", config.api_key.parse()?);
    //request number of results
    let enum_response = request(client.clone(), headers.clone(), enum_query_json, config.clone()).await.expect("test");
    //serialize from string
    let value: serde_json::Value = serde_json::from_str(&enum_response).unwrap();
    //extract the number
    let number_of_results: i64 = value["data"]["phone_numbers"]["page_info"]["total_pages"]
      .as_i64()
      .unwrap();
    //build the query string with the number of results. this is the query we will use to get the contacts.
    let data = format!(r#"
    {{
        "query": "{{\r\n  phone_numbers(paginator: {{page: 1, records_per_page: {}}}) {{\r\n    entities {{\r\n      number\r\n      contact {{\r\n        name\r\n      }}\r\n    }}\r\n  }}\r\n}}"
    }}
    "#, number_of_results.to_string());
    //serialize
    let json: serde_json::Value = serde_json::from_str(&data)?;
    //request contacts
    let response = request(client.clone(), headers.clone(), json, config.clone()).await.expect("test");
    //parse the JSON
    let parsed_response: Root = serde_json::from_str(&response)
        .expect("incorrect JSON formatting. could not deserialize into Root struct");
    //make a vector to store the results
    let mut contacts: Vec<(String, String)> = Vec::new();
    //iterate over entities extracting the name and number into the vec
    for entity in parsed_response.data.phone_numbers.entities {
        let name = entity.contact.name;
        let number = entity.number;
        contacts.push((name, number));
    }
    //init csv writer to the file we want to write
    let mut csv_wtr = Writer::from_path("contacts.csv")?;
    //write headers
    csv_wtr.write_record(&["number", "name"])?;
    //iterate over and write fields
    for (name, number) in &contacts {
        csv_wtr.write_record(&[number, name])?;
    }
    csv_wtr.flush()?;

    Ok(())
}

async fn request(client: Client, headers: HeaderMap, json:serde_json::Value, config: AppConfig) -> Result<std::string::String, reqwest::Error>
     {
        let request = client
        .request(reqwest::Method::GET, config.url)
        .headers(headers)
        .json(&json);

    let response = request.send().await;
    let buffer = response.expect("buffer failed").text().await;
    return buffer;
}

fn load_config() -> Result<AppConfig, ConfigError> {
    let settings = Config::builder()
        .add_source(config::File::with_name("Settings")) // This will look for a file named `Settings.toml` or `Settings.json`, etc.
        .build()?;

    settings.try_deserialize::<AppConfig>()
}

//this is the config struct
#[derive(Debug, Deserialize)]
struct AppConfig {
    url: String,
    api_key: String,
    data: String,
}
//we need clone on this so we can use it in multiple requests
impl Clone for AppConfig {
    fn clone(&self) -> Self {
        AppConfig {
            url: self.url.clone(),
            api_key: self.api_key.clone(),
            data: self.data.clone(),
        }
    }
}

//following structs for json parsing
#[derive(Deserialize, Debug)]
struct Contact {
    name: String,
}

#[derive(Deserialize, Debug)]
struct Entity {
    number: String,
    contact: Contact,
}

#[derive(Deserialize, Debug)]
struct PhoneNumbers {
    entities: Vec<Entity>,
}

#[derive(Deserialize, Debug)]
struct Data {
    phone_numbers: PhoneNumbers,
}

#[derive(Deserialize, Debug)]
struct Root {
    data: Data,
}

//lets write some tests
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_load_config() {
        let config = load_config().unwrap();
        //i cant test values here but i can test keys exist
        assert!(config.url.len() > 0);
        assert!(config.api_key.len() > 0);
        //assert!(config.data.len() > 0);
    }

    #[test]
    fn test_strucs() {
        let test: Root = Root {
            data: Data {
                phone_numbers: PhoneNumbers {
                    entities: vec![Entity {
                        number: "123".to_string(),
                        contact: Contact {
                            name: "test".to_string(),
                        }
                    }]
                }
            }
        };

        assert_eq!(test.data.phone_numbers.entities[0].number, "123");
        assert_eq!(test.data.phone_numbers.entities[0].contact.name, "test");
    }

    #[tokio::test]
    async fn test_request() {
        let config = load_config().unwrap();
        let client = reqwest::Client::builder().build().unwrap();
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("Content-Type", "application/json".parse().unwrap());
        headers.insert("Authorization", config.api_key.parse().unwrap());
        let data = format!(r#"
        {{
            "query": "{{\r\n  phone_numbers(paginator: {{page: 1, records_per_page: {}}}) {{\r\n    entities {{\r\n      number\r\n      contact {{\r\n        name\r\n      }}\r\n    }}\r\n  }}\r\n}}"
        }}
        "#, "1".to_string());
        let json: serde_json::Value = serde_json::from_str(&data).unwrap();
        let response = request(client.clone(), headers.clone(), json, config.clone()).await.expect("test");
        let parsed_response: Root = serde_json::from_str(&response).unwrap();
        let int: i64 = parsed_response.data.phone_numbers.entities[0].number.to_string().parse().unwrap();
        assert_eq!(int > 0, true);
    }
}
