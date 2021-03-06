use std::str::FromStr;
use std::old_io::{File};
use std::os;

use iron::prelude::*;
use iron::{AfterMiddleware, typemap};
use iron::modifier::Modifier;
use plugin::Phantom;
use plugin::Plugin as PluginFor;
use iron::headers;

use glob::glob;

use hyper::header::ContentType;

use handlebars::Handlebars;
use serialize::json::{ToJson, Json};

pub struct HandlebarsEngine {
    registry: Handlebars
}

#[derive(Clone)]
pub struct Template {
    name: String,
    value: Json
}

impl Template {
    pub fn new<T: ToJson>(name: &str, value: T) -> Template {
        Template {
            name: name.to_string(),
            value: value.to_json()
        }
    }
}

impl typemap::Key for HandlebarsEngine {
    type Value = Template;
}

impl Modifier<Response> for Template {
    fn modify(self, resp: &mut Response) {
        resp.extensions.insert::<HandlebarsEngine>(self);
    }
}

impl PluginFor<Response> for HandlebarsEngine {
    fn eval(resp: &mut Response, _: Phantom<HandlebarsEngine>) -> Option<Template> {
        resp.extensions.get::<HandlebarsEngine>().cloned()
    }
}

impl HandlebarsEngine {
    pub fn new(prefix: &str, suffix: &str) -> HandlebarsEngine {
        let mut r = Handlebars::new();

        let prefix_path = Path::new(prefix);
        let abs_prefix_path = os::make_absolute(&prefix_path).unwrap();
        let prefix_path_str = abs_prefix_path.as_str().unwrap();

        let mut pattern = String::new();
        pattern.push_str(prefix_path_str);
        pattern.push_str("/**/*");
        pattern.push_str(suffix);

        for entry in glob(pattern.as_slice()).unwrap() {
            match entry {
                Ok(path) => {
                    let disp = path.as_str().unwrap();
                    let t = r.register_template_string(
                        &disp[prefix_path_str.len()+1 .. disp.len()-suffix.len()],
                        File::open(&path).ok()
                            .expect(format!("Failed to open file {}", disp).as_slice())
                            .read_to_string().unwrap());

                    if t.is_err() {
                        panic!("Failed to create template.");
                    }
                },
                Err(_) => {}
            }
        }

        HandlebarsEngine {
            registry: r
        }
    }
}

impl AfterMiddleware for HandlebarsEngine {
    fn after(&self, _: &mut Request, r: Response) -> IronResult<Response> {
        let mut resp = r;
        // internally we still extensions.get to avoid clone
        let page = match resp.extensions.get::<HandlebarsEngine>() {
            Some(h) => {
                let name = &h.name;
                let value = &h.value;
                let rendered = self.registry.render(name.as_slice(), value);
                match rendered {
                    Ok(r) => Some(r),
                    Err(_) => None
                }
            },
            None => {
                None
            }
        };

        if page.is_some() {
            if !resp.headers.has::<ContentType>() {
                resp.headers.set(headers::ContentType(FromStr::from_str("text/html;charset=utf-8").unwrap()));
            }
            resp.set_mut(page.unwrap());
        }

        Ok(resp)
    }
}

#[cfg(test)]
mod test {
    use serialize::json::ToJson;
    use std::collections::BTreeMap;
    use iron::prelude::*;
    use middleware::*;

    fn hello_world() -> IronResult<Response> {
        let resp = Response::new();

        let mut data = BTreeMap::new();
        data.insert("title".to_string(), "Handlebars on Iron".to_json());

        Ok(resp.set(Template::new("index", data)))
    }

    #[test]
    fn test_resp_set() {
        let mut resp = hello_world().ok().expect("response expected");

        // use response plugin to retrieve a cloned template for testing
        match resp.get::<HandlebarsEngine>() {
            Some(h) => {
                assert_eq!(h.name, "index".to_string());
                assert_eq!(h.value.as_object().unwrap()
                           .get(&"title".to_string()).unwrap()
                           .as_string().unwrap(),
                           "Handlebars on Iron");
            },
            None => panic!("template expected")
        }
    }
}
