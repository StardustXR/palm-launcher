use std::f32::consts::FRAC_PI_2;

use glam::{Quat, Vec3};
use stardust_xr_asteroids::{Context, CreateInnerInfo, CustomElement, FnWrapper, ValidState};
use stardust_xr_fusion::{
    client::FrameInfo,
    spatial::{Spatial, SpatialExt, SpatialRef, Transform},
};
use tracing::info;

/// launches a shell command once at `pos`, upright to the client root and facing
/// sideways to the line from the parent's origin to `pos`
pub struct AppLauncher<State: ValidState> {
    cmd: String,
    pos: Vec3,
    done: FnWrapper<dyn Fn(&mut State) + Send + Sync>,
}
impl<State: ValidState> AppLauncher<State> {
    pub fn new(cmd: impl Into<String>, pos: Vec3) -> Self {
        Self {
            cmd: cmd.into(),
            pos,
            done: FnWrapper(Box::new(|_| {})),
        }
    }
    pub fn done(mut self, f: impl Fn(&mut State) + Send + Sync + 'static) -> Self {
        self.done = FnWrapper(Box::new(f));
        self
    }
}
impl<State: ValidState> std::fmt::Debug for AppLauncher<State> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppLauncher")
            .field("cmd", &self.cmd)
            .field("pos", &self.pos)
            .finish()
    }
}
impl<State: ValidState> CustomElement<State> for AppLauncher<State> {
    type Inner = (Spatial, Option<SpatialRef>);
    type Error = stardust_xr_fusion::Error;

    async fn create_inner(
        &self,
        context: &Context,
        info: CreateInnerInfo,
    ) -> Result<Self::Inner, Self::Error> {
        let client = &context.stardust_client;
        let (spatial, spatial_ref) = Spatial::new(
            client,
            &info.parent_space,
            Transform::from_translation(self.pos),
        )
        .await?;
        let t = spatial
            .get_relative_transform(client.root().clone())
            .await??;
        let d = Quat::from(t.rotation) * self.pos;
        spatial.set_parent(client.root().clone())?;
        spatial.set_local_transform(Transform {
            translation: t.translation,
            rotation: Quat::from_rotation_y(d.x.atan2(d.z) - FRAC_PI_2).into(),
            scale: Vec3::ONE.into(),
        })?;
        Ok((spatial, Some(spatial_ref)))
    }

    fn diff(&self, _old_self: &Self, _context: &Context, _inner: &mut Self::Inner) {}

    fn frame(
        &self,
        context: &Context,
        _info: &FrameInfo,
        state: &mut State,
        inner: &mut Self::Inner,
    ) {
        let Some(spatial_ref) = inner.1.take() else {
            return;
        };
        let client = context.stardust_client.clone();
        let cmd = self.cmd.clone();
        tokio::spawn(async move {
            let Ok(token) = client.generate_startup_token(spatial_ref).await else {
                return;
            };
            info!("launching \"{cmd}\"");
            // TODO: should probably do proper shell parsing on the cmd and
            // launch that directly, this is kinda ugly!
            protostar_launcher::launch(
                ["sh", "-c", &cmd],
                [("STARDUST_STARTUP_TOKEN".to_string(), token)],
            )
            .await;
        });
        (self.done.0)(state);
    }
}
