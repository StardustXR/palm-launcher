use std::{fmt::Debug, future::ready};

use gluon::{Handler, Node, RefExt};
use stardust_xr_asteroids::{CustomElement, FnWrapper, ValidState};
use stardust_xr_fusion::{
    client::FrameInfo,
    spatial::Spatial,
    tracked::{
        Tracked, TrackedExt, TrackedGuard, TrackedStateReceiver, TrackedStateReceiverHandler,
    },
};
use tokio::sync::mpsc;

#[derive(Debug)]
pub struct ExternalSpatialRef<State: ValidState + Debug> {
    tracked_path: String,
    tracked_changed: Option<FnWrapper<dyn Fn(&mut State, bool) + Send + Sync + 'static>>,
}
impl<State: ValidState + Debug> ExternalSpatialRef<State> {
    pub fn new(tracked_service_path: &str) -> Self {
        Self {
            tracked_path: tracked_service_path.to_string(),
            tracked_changed: None,
        }
    }
    pub fn tracked_changed(
        mut self,
        func: impl Fn(&mut State, bool) + Send + Sync + 'static,
    ) -> Self {
        self.tracked_changed.replace(FnWrapper(Box::new(func)));
        self
    }
}
pub struct ExternalSpatialRefInner {
    _spatial: Spatial,
    tracked_changed_recv: mpsc::UnboundedReceiver<bool>,
    _guard: TrackedGuard,
    _node: Node<TrackedSpatialHandler>,
}

impl<State: ValidState + Debug> CustomElement<State> for ExternalSpatialRef<State> {
    type Inner = ExternalSpatialRefInner;

    type Error = stardust_xr_fusion::Error;

    async fn create_inner(
        &self,
        _ctx: &stardust_xr_asteroids::Context,
        info: stardust_xr_asteroids::CreateInnerInfo,
    ) -> Result<Self::Inner, Self::Error> {
        let spatial = info.child_space;
        let (tx, rx) = mpsc::unbounded_channel();
        let tracked = Tracked::binding(&self.tracked_path).await?;
        let (node, recv) = TrackedStateReceiver::new_node(TrackedSpatialHandler { sender: tx })?;
        let (spatial_ref, guard, currently_tracked) = tracked.get(recv.into_proxy()).await?;
        _ = node.sender.send(currently_tracked);
        _ = spatial.set_parent(spatial_ref);
        Ok(ExternalSpatialRefInner {
            _spatial: spatial,
            tracked_changed_recv: rx,
            _guard: guard,
            _node: node,
        })
    }

    fn diff(
        &self,
        _old_self: &Self,
        _context: &stardust_xr_asteroids::Context,
        _inner: &mut Self::Inner,
    ) {
        // TODO: recreate tracked on path change
        // if self.tracked_path != old_self.tracked_path {
        //     let tracked = Tracked::binding(&self.tracked_path).await?;
        //     let (node, recv) =
        //         TrackedStateReceiver::new_node(TrackedSpatialHandler { sender: tx })?;
        //     let (spatial_ref, guard, currently_tracked) = tracked.get(recv.into_proxy()).await?;
        //     _ = node.sender.send(currently_tracked);
        //     _ = inner.spatial.set_parent(spatial_ref);
        // }
    }
    fn frame(
        &self,
        _context: &stardust_xr_asteroids::Context,
        _info: &FrameInfo,
        state: &mut State,
        inner: &mut Self::Inner,
    ) {
        if let Some(func) = self.tracked_changed.as_ref() {
            while let Ok(tracked) = inner.tracked_changed_recv.try_recv() {
                func.0(state, tracked);
            }
        }
    }
}

#[derive(Handler)]
struct TrackedSpatialHandler {
    sender: mpsc::UnboundedSender<bool>,
}
impl TrackedStateReceiverHandler for TrackedSpatialHandler {
    fn tracked(
        &self,
        _ctx: gluon::Context,
        tracked: bool,
    ) -> impl Future<Output = ()> + Send + Sync {
        _ = self.sender.send(tracked);
        ready(())
    }
}
