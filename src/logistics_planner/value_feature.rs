use std::sync::Arc;
use vrp_core::models::common::Dimensions;
use vrp_core::models::problem::Single;
use vrp_core::prelude::*;
use vrp_core::utils::Float;

vrp_core::custom_dimension!(JobValue typeof f64);

pub fn feature_layer() -> Feature {
    create_maximize_total_job_value_feature(
        "max_value",
        Arc::new(move |job| job.dimens().get_job_value().copied().unwrap_or(0.0)),
        Arc::new(|job, value| match job {
            Job::Single(single) => {
                let mut dimens = single.dimens.clone();
                dimens.set_job_value(value);

                Job::Single(Arc::new(Single {
                    places: single.places.clone(),
                    dimens,
                }))
            }
            _ => job.clone(),
        }),
    )
    .expect("Failed to create max value feature")
}

/// Specifies a job value reader as a variant of two functions.
pub type JobReadValueFn = Arc<dyn Fn(&Job) -> Float + Send + Sync>;
/// Specifies a job write value.
pub type JobWriteValueFn = Arc<dyn Fn(Job, Float) -> Job + Send + Sync>;
/// A job value estimation function.
type EstimateValueFn = Arc<dyn Fn(&RouteContext, &Job) -> Float + Send + Sync>;

/// Maximizes a total value of served jobs.
pub fn create_maximize_total_job_value_feature(
    name: &str,
    job_read_value_fn: JobReadValueFn,
    job_write_value_fn: JobWriteValueFn,
) -> Result<Feature, GenericError> {
    FeatureBuilder::default()
        .with_name(name)
        .with_objective(MaximizeTotalValueObjective {
            estimate_value_fn: Arc::new({
                let job_read_value_fn = job_read_value_fn.clone();
                let sign = -1.;
                move |_route_ctx, job| sign * (job_read_value_fn)(job)
            }),
        })
        .with_constraint(MaximizeTotalValueConstraint {
            job_read_value_fn,
            job_write_value_fn,
        })
        .build()
}

struct MaximizeTotalValueObjective {
    estimate_value_fn: EstimateValueFn,
}

impl FeatureObjective for MaximizeTotalValueObjective {
    fn fitness(&self, solution: &InsertionContext) -> Cost {
        solution.solution.routes.iter().fold(0., |acc, route_ctx| {
            route_ctx.route().tour.jobs().fold(acc, |acc, job| {
                acc + (self.estimate_value_fn)(route_ctx, job)
            })
        })
    }

    fn estimate(&self, move_ctx: &MoveContext<'_>) -> Cost {
        match move_ctx {
            MoveContext::Route { route_ctx, job, .. } => (self.estimate_value_fn)(route_ctx, job),
            MoveContext::Activity { .. } => Cost::default(),
        }
    }
}

struct MaximizeTotalValueConstraint {
    job_read_value_fn: JobReadValueFn,
    job_write_value_fn: JobWriteValueFn,
}

impl FeatureConstraint for MaximizeTotalValueConstraint {
    fn evaluate(&self, _: &MoveContext<'_>) -> Option<ConstraintViolation> {
        None
    }

    fn merge(&self, source: Job, candidate: Job) -> Result<Job, ViolationCode> {
        let source_value = (self.job_read_value_fn)(&source);
        let candidate_value = (self.job_read_value_fn)(&candidate);
        let new_value = source_value + candidate_value;

        Ok(if new_value != source_value {
            (self.job_write_value_fn)(source, new_value)
        } else {
            source
        })
    }
}
