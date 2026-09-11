pub mod derezzer;
pub mod spatial_ref;
pub mod spatial_ref_exposer;

use std::{
    env,
    f32::consts::{FRAC_PI_2, FRAC_PI_3, PI},
    str::FromStr,
};

use glam::{Quat, Vec3, vec3};
use serde::{Deserialize, Serialize};
use stardust_xr_asteroids::{
    ClientState, Context, CustomElement, Entity, Migrate, Reify, Tasker, Transformable,
    client::run,
    components::Grabbable,
    elements::{Lines, Spatial, Text},
};
use stardust_xr_fusion::{
    client::FrameInfo,
    drawable::{Line, LinePoint},
    fields::Shape,
    spatial::{PartialTransform, SpatialExt, SpatialRef, Transform},
    types::rgba_linear,
};

use crate::{
    derezzer::Derezzer, spatial_ref::ExternalSpatialRef, spatial_ref_exposer::SpatialRefExposer,
};

#[tokio::main]
async fn main() {
    run::<PalmLauncher>(&[]).await.unwrap();
}
#[derive(Debug, Serialize, Deserialize, Default)]
enum Action {
    #[default]
    Nothing,
    Command(String),
    Destroy,
}
#[derive(Debug, Serialize, Deserialize, Default)]
enum Target {
    #[default]
    HandLeft,
    HandRight,
    ControllerLeft,
    ControllerRight,
}
impl FromStr for Target {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Hand/Left" => Ok(Self::HandLeft),
            "Hand/Right" => Ok(Self::HandRight),
            "Controller/Left" => Ok(Self::ControllerLeft),
            "Controller/Right" => Ok(Self::ControllerRight),
            _ => Err(s.to_string()),
        }
    }
}
impl Target {
    fn spatial_ref_info(&self) -> &'static str {
        match self {
            Target::HandLeft => "stardust-hand/left",
            Target::HandRight => "stardust-hand/right",
            Target::ControllerLeft => "stardust-controller/left",
            Target::ControllerRight => "stardust-controller/right",
        }
    }
    fn offset(&self) -> (Vec3, Quat) {
        match self {
            Target::HandLeft => (
                vec3(0.0, -0.02, 0.0),
                Quat::from_rotation_x(FRAC_PI_2) * Quat::from_rotation_z(FRAC_PI_2),
            ),
            Target::HandRight => (
                vec3(0.0, -0.02, 0.0),
                Quat::from_rotation_x(FRAC_PI_2) * Quat::from_rotation_z(FRAC_PI_2),
            ),
            Target::ControllerLeft => (vec3(0.0, -0.01, 0.01), Quat::from_rotation_x(-FRAC_PI_3)),
            Target::ControllerRight => todo!(),
        }
    }
    fn text_rot(&self) -> Quat {
        match self {
            Target::HandLeft => Quat::from_rotation_z(FRAC_PI_2),
            Target::HandRight => Quat::from_rotation_z(-FRAC_PI_2) * Quat::from_rotation_x(PI),
            Target::ControllerLeft => Quat::from_rotation_z(FRAC_PI_2),
            Target::ControllerRight => todo!(),
        }
    }
}
#[derive(Debug, Serialize, Deserialize, Default)]
struct PalmLauncher {
    target: Target,
    pos: Vec3,
    rot: Quat,
    state: Action,
    #[serde(skip)]
    handle_ref: Option<SpatialRef>,
    commands: Vec<String>,
    visible: bool,
}

impl Reify for PalmLauncher {
    fn reify(
        &self,
        context: &Context,
        _tasks: impl Tasker<Self>,
    ) -> impl stardust_xr_asteroids::Element<Self> {
        let name = self.target.spatial_ref_info();
        // TODO: the coordinate space this makes is kinda terrible, should probabl be fixed
        let (pos, rot) = self.target.offset();
        ExternalSpatialRef::new(name)
            .tracked_changed(|state: &mut PalmLauncher, tracked| {
                println!("tracked state changed: {tracked}");
                state.visible = tracked;
                state.pos = Vec3::ZERO;
                state.rot = Quat::IDENTITY;
                state.state = Action::Nothing;
            })
            .build()
            // .child(Axes::default().build())
            .maybe_child(self.visible.then(|| {
                let client = context.stardust_client.clone();
                Spatial::default()
                    .pos(pos)
                    .rot(rot)
                    .build()
                    // .child(Axes::default().build())
                    .child(
                        Lines::new([Line {
                            points: {
                                let color = match &self.state {
                                    Action::Nothing => {
                                        context.accent_color.color()
                                            * rgba_linear!(0.1, 0.1, 0.1, 1.0)
                                    }
                                    Action::Command(_) => {
                                        context.accent_color.color()
                                            * rgba_linear!(0.8, 0.8, 0.8, 1.0)
                                    }
                                    Action::Destroy => rgba_linear!(1., 0., 0., 1.),
                                };
                                vec![
                                    LinePoint {
                                        point: Vec3::ZERO.into(),
                                        thickness: 0.001,
                                        color,
                                    },
                                    LinePoint {
                                        point: self.pos.into(),
                                        thickness: 0.001,
                                        color,
                                    },
                                ]
                            },
                            cyclic: false,
                        }])
                        .build(),
                    )
                    .maybe_child(if let Action::Command(cmd) = &self.state {
                        let quat = Quat::from_rotation_arc(Vec3::Y, self.pos.normalize())
                            * self.target.text_rot();
                        Some(
                            Text::new(cmd)
                                .rot(quat)
                                .pos((self.pos * 0.5) + (quat.mul_vec3(Vec3::Y * 0.01)))
                                .build(),
                        )
                    } else {
                        None
                    })
                    .child(
                        SpatialRefExposer::new(|state: &mut Self, spatial_ref| {
                            state.handle_ref = Some(spatial_ref)
                        })
                        .build(),
                    )
                    .maybe_child(matches!(self.state, Action::Destroy).then(|| {
                        Derezzer::new(
                            Vec3::ZERO,
                            Quat::from_rotation_arc(Vec3::NEG_Z, self.pos.normalize()),
                            self.pos.length(),
                        )
                        .build()
                    }))
                    .child(
                        Entity::new(Shape::Cylinder {
                            length: 0.02,
                            radius: 0.002,
                        })
                        .pos(self.pos)
                        .rot(self.rot)
                        .component(
                            Grabbable::new(|state: &mut PalmLauncher, pose| {
                                state.pos = pose.position.into();
                                state.rot = pose.orientation.into()
                            })
                            .max_distance(0.025)
                            .grab_stop(
                                move |state: &mut PalmLauncher| {
                                    let client = client.clone();
                                    if let Action::Command(cmd) = &state.state {
                                        let cmd = cmd.clone();
                                        let spatial_ref = state.handle_ref.clone().unwrap();
                                        let pos = state.pos;
                                        tokio::spawn(async move {
                                            let (spatial, spatial_ref) =
                                                stardust_xr_fusion::spatial::Spatial::new(
                                                    &client,
                                                    &spatial_ref,
                                                    Transform::from_translation(pos * 0.5),
                                                )
                                                .await
                                                .unwrap();

                                            _ = spatial.set_relative_transform(
                                                client.root().clone(),
                                                PartialTransform::from_rotation_scale(
                                                    Quat::IDENTITY,
                                                    Vec3::ONE,
                                                ),
                                            );
                                            let token = client
                                                .generate_startup_token(spatial_ref)
                                                .await
                                                .unwrap();
                                            // TODO: should this use "sh -c {cmd}" to run the thingy?
                                            protostar_launcher::launch(
                                                cmd.into(),
                                                [("STARDUST_STARTUP_TOKEN".to_string(), token)],
                                            )
                                            .await;
                                        });
                                    }
                                    state.pos = Vec3::ZERO;
                                    state.rot = Quat::IDENTITY;
                                    println!("grab stopped");
                                    state.state = Action::Nothing;
                                },
                            ),
                        )
                        .build()
                        .child(
                            Lines::new([Line {
                                points: vec![
                                    LinePoint {
                                        point: vec3(0.0, -0.01, 0.0).into(),
                                        thickness: 0.002,
                                        color: rgba_linear!(1., 1., 1., 1.),
                                    },
                                    LinePoint {
                                        point: vec3(0.0, 0.01, 0.0).into(),
                                        thickness: 0.002,
                                        color: rgba_linear!(1., 1., 1., 1.),
                                    },
                                ],
                                cyclic: false,
                            }])
                            .build(),
                        ),
                    )
            }))
    }
}
impl ClientState for PalmLauncher {
    const APP_ID: &'static str = "dev.schmarni.palmlauncher";

    fn initial_state_update(&mut self) {
        let mut args = env::args().into_iter().skip(1);
        let target = Target::from_str(
            &args
                .next()
                .expect("no target specified, use Hand/Left Conroller/Right etc"),
        )
        .expect("invalid_target_specified, use Hand/Left Conroller/Right etc");
        let args = args.collect();
        self.target = target;
        self.commands = args;
    }

    fn on_frame(&mut self, _info: &FrameInfo) {
        let v = 0.5 / self.commands.len() as f32;
        let index = (self.pos.length() / v).floor() as usize;
        self.state = if index == 0 {
            Action::Nothing
        } else if index > self.commands.len() {
            Action::Destroy
        } else {
            let index = index - 1;
            Action::Command(self.commands[index].clone())
        }
    }
}
impl Migrate for PalmLauncher {
    type Old = Self;
}
