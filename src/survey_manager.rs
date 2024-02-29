use crate::data::DataClient;
use crate::models::{KeyedSurvey, Survey, WaypointSymbol};
use chrono::Duration;
use std::collections::{BTreeMap, VecDeque};
use std::sync::Mutex;

pub struct SurveyManager {
    db: DataClient,
    inner: Mutex<SurveyManagerInner>,
}

struct SurveyManagerInner {
    surveys: BTreeMap<WaypointSymbol, VecDeque<KeyedSurvey>>,
}

impl SurveyManager {
    pub async fn new(db: &DataClient) -> Self {
        let surveys = db.get_surveys().await;
        let surveys = surveys
            .into_iter()
            .fold(BTreeMap::new(), |mut map, survey| {
                map.entry(survey.survey.symbol.clone())
                    .or_insert_with(VecDeque::new)
                    .push_back(survey);
                map
            });
        Self {
            db: db.clone(),
            inner: Mutex::new(SurveyManagerInner { surveys }),
        }
    }

    pub async fn insert_surveys(&self, surveys: Vec<Survey>) {
        let surveys = surveys
            .into_iter()
            .map(|survey| KeyedSurvey {
                uuid: uuid::Uuid::new_v4(),
                survey,
            })
            .collect();
        self.db.insert_surveys(&surveys).await;
        let mut inner = self.inner.lock().unwrap();
        for survey in surveys {
            inner
                .surveys
                .entry(survey.survey.symbol.clone())
                .or_insert_with(VecDeque::new)
                .push_back(survey);
        }
    }

    pub async fn get_survey(&self, waypoint: &WaypointSymbol) -> Option<KeyedSurvey> {
        // todo: better selection of a survey for specific resources

        let now = chrono::Utc::now();
        loop {
            // grab front
            let front = {
                let mut inner = self.inner.lock().unwrap();
                let surveys = inner.surveys.entry(waypoint.clone()).or_default();
                surveys.front().cloned()
            };
            // delete or return
            if let Some(survey) = front {
                if survey.survey.expiration + Duration::minutes(5) < now {
                    self.remove_survey(&survey).await;
                } else {
                    return Some(survey.clone());
                }
            } else {
                return None;
            }
        }
    }

    pub async fn remove_survey(&self, survey: &KeyedSurvey) {
        log::debug!("Deleting survey {}", survey.uuid);
        self.db.remove_survey(&survey.uuid).await;

        let mut inner = self.inner.lock().unwrap();
        inner
            .surveys
            .entry(survey.survey.symbol.clone())
            .and_modify(|v| {
                v.retain(|s| s.uuid != survey.uuid);
            });
    }
}
