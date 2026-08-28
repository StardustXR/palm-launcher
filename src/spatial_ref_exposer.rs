use std::fmt::Debug;

use stardust_xr_asteroids::{CustomElement, FnWrapper, ValidState};
use stardust_xr_fusion::{client::FrameInfo, spatial::SpatialRef};

#[derive(Debug)]
pub struct SpatialRefExposer<State: ValidState + Debug>(
    FnWrapper<dyn Fn(&mut State, SpatialRef) + Send + Sync>,
);

impl<State: ValidState + Debug> SpatialRefExposer<State> {
    pub fn new(callback: impl Fn(&mut State, SpatialRef) + Send + Sync + 'static) -> Self {
        Self(FnWrapper(Box::new(callback)))
    }
}
impl<State: ValidState + Debug> CustomElement<State> for SpatialRefExposer<State> {
    type Inner = SpatialRef;

    type Error = stardust_xr_fusion::Error;

    async fn create_inner(
        &self,
        _asteroids_context: &stardust_xr_asteroids::Context,
        info: stardust_xr_asteroids::CreateInnerInfo,
    ) -> Result<Self::Inner, Self::Error> {
        Ok(info.child_space.spatial_ref().await?)
    }

    fn diff(
        &self,
        _old_self: &Self,
        _asteroids_context: &stardust_xr_asteroids::Context,
        _inner: &mut Self::Inner,
    ) {
    }

    fn frame(
        &self,
        _context: &stardust_xr_asteroids::Context,
        _info: &FrameInfo,
        state: &mut State,
        inner: &mut Self::Inner,
    ) {
        self.0.0(state, inner.clone());
    }
}
