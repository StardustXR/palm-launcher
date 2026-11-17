pub mod app_launcher;

use std::{
    env,
    f32::consts::{FRAC_PI_2, FRAC_PI_3},
    str::FromStr,
};

use glam::{Quat, Vec3, vec3};
use serde::{Deserialize, Serialize};
use stardust_xr_asteroids::{
    ClientState, Context, CustomElement, Entity, Migrate, Reify, Tasker, Transformable,
    client::run,
    components::Grabbable,
    elements::{BeamQuery, Lines, Spatial, Text, Tracked},
};
use stardust_xr_fusion::{
    client::FrameInfo,
    drawable::{Line, LinePoint},
    fields::Shape,
    types::rgba_linear,
};
use stardust_xr_molecules::derezzable::protocol::Derezzable;
use tracing::debug;

use crate::app_launcher::AppLauncher;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    run::<PalmLauncher>(&[]).await.unwrap();
}
#[derive(Debug, Serialize, Deserialize, Default)]
enum Action {
    #[default]
    Nothing,
    Command(String),
    Derez,
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
            Target::HandLeft => "stardust-hand/palm/left",
            Target::HandRight => "stardust-hand/palm/right",
            // TODO: maybe attach this to grip_surface?
            Target::ControllerLeft => "stardust-controller/grip/left",
            Target::ControllerRight => "stardust-controller/grip/right",
        }
    }
    fn mirrored(&self) -> bool {
        matches!(self, Target::HandRight | Target::ControllerRight)
    }
    /// offsets are authored for the left side, openxr's right side spaces are the left's
    /// mirrored with their local X flipped, so mirroring across X gives the matching pose
    fn mirror(&self, p: Vec3, r: Quat) -> (Vec3, Quat) {
        if self.mirrored() {
            (vec3(-p.x, p.y, p.z), Quat::from_xyzw(r.x, -r.y, -r.z, r.w))
        } else {
            (p, r)
        }
    }
    fn offset(&self) -> (Vec3, Quat) {
        match self {
            Target::HandLeft | Target::HandRight => self.mirror(
                vec3(0.0, -0.03, 0.0),
                Quat::from_rotation_x(FRAC_PI_2) * Quat::from_rotation_z(FRAC_PI_2),
            ),
            Target::ControllerLeft | Target::ControllerRight => self.mirror(
                vec3(0.0, -0.01, 0.01),
                Quat::from_rotation_x(-FRAC_PI_3 - FRAC_PI_2),
            ),
        }
    }
    fn text_rot(&self) -> Quat {
        self.mirror(Vec3::ZERO, Quat::from_rotation_z(FRAC_PI_2)).1
    }
}
#[derive(Debug, Serialize, Deserialize, Default)]
struct PalmLauncher {
    target: Target,
    pos: Vec3,
    rot: Quat,
    state: Action,
    #[serde(skip)]
    launching: Option<(String, Vec3)>,
    commands: Vec<String>,
    visible: bool,
}

impl Reify for PalmLauncher {
    fn reify(
        &self,
        context: &Context,
        _tasks: impl Tasker<Self>,
        _props: (),
    ) -> impl stardust_xr_asteroids::Element<Self> {
        let name = self.target.spatial_ref_info();
        // TODO: the coordinate space this makes is kinda terrible, should probabl be fixed
        let (pos, rot) = self.target.offset();
        Tracked::new(name, |state: &mut PalmLauncher, tracked| {
            debug!("tracked state changed: {tracked}");
            state.visible = tracked;
            state.pos = Vec3::ZERO;
            state.rot = Quat::IDENTITY;
            state.state = Action::Nothing;
        })
        .build()
        // .child(Axes::default().build())
        .maybe_child(self.visible.then(|| {
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
                                    context.accent_color.color() * rgba_linear!(0.1, 0.1, 0.1, 1.0)
                                }
                                Action::Command(_) => {
                                    context.accent_color.color() * rgba_linear!(0.8, 0.8, 0.8, 1.0)
                                }
                                Action::Derez => rgba_linear!(1., 0., 0., 1.),
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
                .maybe_child(self.launching.as_ref().map(|(cmd, pos)| {
                    AppLauncher::new(cmd, *pos)
                        .done(|state: &mut Self| state.launching = None)
                        .build()
                }))
                .maybe_child(matches!(self.state, Action::Derez).then(|| {
                    BeamQuery::new(
                        Vec3::ZERO,
                        self.pos.normalize(),
                        |_: &mut Self, _, _, _, (derezzable,): (Derezzable,), _| {
                            _ = derezzable.derez();
                        },
                        |_, _, _| {},
                        |_, _, _| {},
                        |_, _| {},
                    )
                    .max_length(self.pos.length())
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
                        .max_distance(0.03f32)
                        .grab_stop(|state: &mut PalmLauncher| {
                            if let Action::Command(cmd) = &state.state {
                                state.launching = Some((cmd.clone(), state.pos * 0.5));
                            }
                            state.pos = Vec3::ZERO;
                            state.rot = Quat::IDENTITY;
                            debug!("grab stopped");
                            state.state = Action::Nothing;
                        }),
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
    const APP_ID: &'static str = "org.stardustxr.PalmLauncher";

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

    fn on_frame(&mut self, _ctx: &Context, _info: &FrameInfo) {
        let v = 0.5 / self.commands.len() as f32;
        let index = (self.pos.length() / v).floor() as usize;
        self.state = if index == 0 {
            Action::Nothing
        } else if index > self.commands.len() {
            Action::Derez
        } else {
            let index = index - 1;
            Action::Command(self.commands[index].clone())
        }
    }
}
impl Migrate for PalmLauncher {
    type Old = Self;
}
