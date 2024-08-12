use std::collections::HashMap;
use std::sync::Arc;
use reqwest::Client;
use serde_json::{json, Value};
pub struct GoogleModule{
    api_key:Option<String>,
    client:Client
}
use base64::{decode, DecodeError};
use crate::base::get_nowtime_str;
use crate::models::MyError;

fn decode_audio(audio_content_base64: &str) -> Result<Vec<u8>, MyError> {
    decode(audio_content_base64).map_err(|e|{
        let str_error = format!("SPEACH|| {} error: Decode speach\n", get_nowtime_str());
        MyError::SiteError(str_error)
    })
}


pub fn get_language_code(language: String) -> Option<String> {
    let language_map: HashMap<&str, &str> = [
        ("Arabic", "ar-SA"),
        ("Dutch", "nl-NL"),
        ("English", "en-US"),
        ("French", "fr-FR"),
        ("German", "de-DE"),
        ("Greek", "el-GR"),
        ("Hebrew", "he-IL"),
        ("Italian", "it-IT"),
        ("Japanese", "ja-JP"),
        ("Korean", "ko-KR"),
        ("Polish", "pl-PL"),
        ("Portuguese", "pt-PT"),
        ("Russian", "ru-RU"),
        ("Spanish", "es-ES"),
        ("Swedish", "sv-SE"),
        ("Thai", "th-TH"),
        ("Turkish", "tr-TR"),
        ("Ukrainian", "uk-UA"),
        ("Vietnamese", "vi-VN"),
    ].iter().cloned().collect();
    let res=language_map.get(language.as_str()).copied();
    match res {
        None => {
            None
        }
        Some(str) => {
            Some(String::from(str))
        }
    }

}
impl GoogleModule{
    pub fn new()->GoogleModule{
        Self{
            api_key:None,
            client:Client::new()
        }
    }
    pub fn init(api:String)->GoogleModule{
        Self{
            api_key:Some(api),
            client:Client::new()
        }
    }
    pub async fn text_to_speach(google_module:Arc<GoogleModule>,text:String,name_lang:String)->Result<Vec<u8>,MyError> {
        let url = "https://texttospeech.googleapis.com/v1/text:synthesize";
        //sv-SE
        let body = json!({
        "input": {
            "text": text
        },
        "voice": {
            "languageCode": get_language_code(name_lang).unwrap(),
            "ssmlGender": "FEMALE"
        },
        "audioConfig": {
            "audioEncoding": "MP3"
        }
    });

        let response = google_module.client.post(url)
            .query(&[("key", google_module.api_key.as_ref())])
            .json(&body)
            .send()
            .await.map_err(|e|{
            let str_error = format!("SPEACH|| {} error: send to speach\n", get_nowtime_str());
            MyError::SiteError(str_error)
        })?;

        if response.status().is_success() {
            let audio_content = response.text().await.map_err(|e|{
                let str_error = format!("SPEACH|| {} error: error content speach\n", get_nowtime_str());
                MyError::SiteError(str_error)
            })?;
            let parsed_json: Value = serde_json::from_str(audio_content.as_str()).map_err(|e|{
                let str_error = format!("SPEACH|| {} error: error parse speach\n", get_nowtime_str());
                MyError::SiteError(str_error)
            })?;
            let audio_content = parsed_json["audioContent"].as_str().unwrap_or("");
            decode_audio(audio_content)
        } else {
            println!("Failed to call API: {}", response.status());
            let str_error = format!("SPEACH|| {} error: error send speach\n", get_nowtime_str());
            Err(MyError::SiteError(str_error))
        }
    }
}