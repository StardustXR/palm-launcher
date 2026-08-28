use std::future::ready;

use glam::Vec3;
use gluon::{Handler, Interface, Node, RefExt};
use stardust_xr_asteroids::{Context, CustomElement, ValidState};
use stardust_xr_fusion::{
    fields::RayMarchResult,
    query::{InterfaceDependency, QueriedInterface, QueryableId},
    spatial::{PartialTransform, Spatial},
    spatial_query::{BeamQuery, BeamQueryHandle, BeamQueryHandler, BeamQueryHandlerHandler},
    types::{QuatF, Vec3F},
};
use stardust_xr_molecules::derezzable::protocol::Derezzable;

#[derive(Debug)]
pub struct Derezzer {
    pos: Vec3F,
    rot: QuatF,
    length: f32,
}
impl Derezzer {
    pub fn new(pos: impl Into<Vec3F>, rot: impl Into<QuatF>, length: f32) -> Self {
        Self {
            pos: pos.into(),
            rot: rot.into(),
            length,
        }
    }
}

impl<State: ValidState> CustomElement<State> for Derezzer {
    type Inner = DerezzerInner;

    type Error = stardust_xr_fusion::Error;

    async fn create_inner(
        &self,
        ctx: &Context,
        info: stardust_xr_asteroids::CreateInnerInfo,
    ) -> Result<Self::Inner, Self::Error> {
        let client = ctx.stardust_client.clone();
        let (query_handler, query_ref) = BeamQueryHandler::new_node(DerezzerQuery)?;
        let query_handle = client
            .spatial_query_interface()
            .beam_query(BeamQuery {
                handler: query_ref.into_proxy(),
                interfaces: vec![InterfaceDependency {
                    id: Derezzable::ID.into(),
                    optional: false,
                }],
                reference_spatial: info.child_space.spatial_ref().await?,
                origin: Vec3::ZERO.into(),
                direction: Vec3::Y.into(),
                max_length: self.length,
            })
            .await?
            // TODO: replace this?
            .unwrap();
        Ok(DerezzerInner {
            spatial: info.child_space,
            query_handler,
            query_handle,
        })
    }

    fn diff(&self, old_self: &Self, _context: &Context, inner: &mut Self::Inner) {
        if self.pos != old_self.pos {
            _ = inner
                .spatial
                .set_local_transform(PartialTransform::from_translation(self.pos));
        }
        if self.rot != old_self.rot {
            _ = inner
                .spatial
                .set_local_transform(PartialTransform::from_rotation(self.rot));
        }
        if self.length != old_self.length {
            _ = inner
                .query_handle
                .update(Vec3::ZERO.into(), Vec3::Y.into(), self.length);
        }
    }
}

pub struct DerezzerInner {
    spatial: Spatial,
    query_handler: Node<DerezzerQuery>,
    query_handle: BeamQueryHandle,
}
#[derive(Handler)]
pub struct DerezzerQuery;
impl BeamQueryHandlerHandler for DerezzerQuery {
    fn intersected(
        &self,
        _ctx: gluon::Context,
        _obj: stardust_xr_fusion::query::QueryableId,
        _field: stardust_xr_fusion::fields::FieldRef,
        _spatial: stardust_xr_fusion::spatial::SpatialRef,
        interfaces: Vec<stardust_xr_fusion::query::QueriedInterface>,
        _spatial_info: stardust_xr_fusion::fields::RayMarchResult,
    ) -> impl Future<Output = ()> {
        let Some(derezzable) = interfaces
            .into_iter()
            .find(|v| v.interface_id == Derezzable::ID)
            .map(|v| Derezzable::from_ref(v.interface))
        else {
            return ready(());
        };
        _ = derezzable.derez();
        ready(())
    }

    fn interfaces_changed(
        &self,
        _ctx: gluon::Context,
        _obj: QueryableId,
        _interfaces: Vec<QueriedInterface>,
    ) -> impl Future<Output = ()> + Send + Sync {
        ready(())
    }

    fn moved(
        &self,
        _ctx: gluon::Context,
        _obj: QueryableId,
        _spatial_info: RayMarchResult,
    ) -> impl Future<Output = ()> + Send + Sync {
        ready(())
    }

    fn left(
        &self,
        _ctx: gluon::Context,
        _obj: QueryableId,
    ) -> impl Future<Output = ()> + Send + Sync {
        ready(())
    }
}
