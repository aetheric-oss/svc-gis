//! Client Library: Client Functions, Structs, Traits

/// gRPC object traits to provide wrappers for grpc functions
#[tonic::async_trait]
pub trait Client<T>
where
    Self: Sized + lib_common::grpc::Client<T> + lib_common::grpc::ClientConnect<T>,
    T: Send + Clone,
{
    /// The type expected for ReadyRequest structs.
    type ReadyRequest;
    /// The type expected for ReadyResponse structs.
    type ReadyResponse;

    /// Returns a [`tonic::Response`] containing a [`ReadyResponse`](Self::ReadyResponse)
    /// Takes an [`ReadyRequest`](Self::ReadyRequest).
    ///
    /// # Errors
    ///
    /// Returns [`tonic::Status`] with [`Code::Unknown`](tonic::Code::Unknown) if
    /// the server is not ready.
    ///
    /// # Examples
    /// ```
    /// use lib_common::grpc::get_endpoint_from_env;
    /// use svc_gis_client_grpc::prelude::*;
    ///
    /// async fn example () -> Result<(), Box<dyn std::error::Error>> {
    ///     let (host, port) = get_endpoint_from_env("SERVER_HOSTNAME", "SERVER_PORT_GRPC");
    ///     let client = GisClient::new_client(&host, port, "gis");
    ///     let response = client
    ///         .is_ready(gis::ReadyRequest {})
    ///         .await?;
    ///     println!("RESPONSE={:?}", response.into_inner());
    ///     Ok(())
    /// }
    /// ```
    async fn is_ready(
        &self,
        request: Self::ReadyRequest,
    ) -> Result<tonic::Response<Self::ReadyResponse>, tonic::Status>;

    /// Returns a [`tonic::Response`] containing a [`UpdateResponse`](super::UpdateResponse)
    /// Takes an [`UpdateWaypointsRequest`](super::UpdateWaypointsRequest).
    ///
    /// # Errors
    ///
    /// Returns [`tonic::Status`] with [`Code::Unknown`](tonic::Code::Unknown) if
    /// the server is not ready.
    ///
    /// # Examples
    /// ```
    /// use lib_common::grpc::get_endpoint_from_env;
    /// use svc_gis_client_grpc::prelude::*;
    ///
    /// async fn example () -> Result<(), Box<dyn std::error::Error>> {
    ///     let (host, port) = get_endpoint_from_env("SERVER_HOSTNAME", "SERVER_PORT_GRPC");
    ///     let client = GisClient::new_client(&host, port, "gis");
    ///     let request = gis::UpdateWaypointsRequest { waypoints: vec![] };
    ///     let response = client.update_waypoints(request).await?;
    ///     println!("RESPONSE={:?}", response.into_inner());
    ///     Ok(())
    /// }
    /// ```
    async fn update_waypoints(
        &self,
        request: super::UpdateWaypointsRequest,
    ) -> Result<tonic::Response<super::UpdateResponse>, tonic::Status>;

    /// Returns a [`tonic::Response`] containing a [`UpdateResponse`](super::UpdateResponse)
    /// Takes an [`UpdateVertiportsRequest`](super::UpdateVertiportsRequest).
    ///
    /// # Errors
    ///
    /// Returns [`tonic::Status`] with [`Code::Unknown`](tonic::Code::Unknown) if
    /// the server is not ready.
    ///
    /// # Examples
    /// ```
    /// use lib_common::grpc::get_endpoint_from_env;
    /// use svc_gis_client_grpc::prelude::*;
    ///
    /// async fn example () -> Result<(), Box<dyn std::error::Error>> {
    ///     let (host, port) = get_endpoint_from_env("SERVER_HOSTNAME", "SERVER_PORT_GRPC");
    ///     let client = GisClient::new_client(&host, port, "gis");
    ///     let request = gis::UpdateVertiportsRequest { vertiports: vec![] };
    ///     let response = client.update_vertiports(request).await?;
    ///     println!("RESPONSE={:?}", response.into_inner());
    ///     Ok(())
    /// }
    /// ```
    async fn update_vertiports(
        &self,
        request: super::UpdateVertiportsRequest,
    ) -> Result<tonic::Response<super::UpdateResponse>, tonic::Status>;

    /// Returns a [`tonic::Response`] containing a [`UpdateResponse`](super::UpdateResponse)
    /// Takes an [`UpdateZonesRequest`](super::UpdateZonesRequest).
    ///
    /// # Errors
    ///
    /// Returns [`tonic::Status`] with [`Code::Unknown`](tonic::Code::Unknown) if
    /// the server is not ready.
    ///
    /// # Examples
    /// ```
    /// use lib_common::grpc::get_endpoint_from_env;
    /// use svc_gis_client_grpc::prelude::*;
    ///
    /// async fn example () -> Result<(), Box<dyn std::error::Error>> {
    ///     let (host, port) = get_endpoint_from_env("SERVER_HOSTNAME", "SERVER_PORT_GRPC");
    ///     let client = GisClient::new_client(&host, port, "gis");
    ///     let request = gis::UpdateZonesRequest { zones: vec![] };
    ///     let response = client.update_zones(request).await?;
    ///     println!("RESPONSE={:?}", response.into_inner());
    ///     Ok(())
    /// }
    /// ```
    async fn update_zones(
        &self,
        request: super::UpdateZonesRequest,
    ) -> Result<tonic::Response<super::UpdateResponse>, tonic::Status>;

    /// Returns a [`tonic::Response`] containing a [`UpdateResponse`](super::UpdateResponse)
    /// Takes an [`UpdateFlightPathRequest`](super::UpdateFlightPathRequest).
    ///
    /// # Errors
    ///
    /// Returns [`tonic::Status`] with [`Code::Unknown`](tonic::Code::Unknown) if
    /// the server is not ready.
    ///
    /// # Examples
    /// ```
    /// use lib_common::grpc::get_endpoint_from_env;
    /// use svc_gis_client_grpc::prelude::*;
    /// use lib_common::time::Utc;
    ///
    /// async fn example () -> Result<(), Box<dyn std::error::Error>> {
    ///     let (host, port) = get_endpoint_from_env("SERVER_HOSTNAME", "SERVER_PORT_GRPC");
    ///     let client = GisClient::new_client(&host, port, "gis");
    ///     let request = gis::UpdateFlightPathRequest {
    ///         flight_identifier: Some("flight-x".to_string()),
    ///         aircraft_identifier: Some("aircraft-x".to_string()),
    ///         simulated: false,
    ///         aircraft_type: AircraftType::Rotorcraft as i32,
    ///         timestamp_start: Some(Utc::now().into()),
    ///         timestamp_end: Some(Utc::now().into()),
    ///         path: vec![],
    ///     };
    ///     let response = client.update_flight_path(request).await?;
    ///     println!("RESPONSE={:?}", response.into_inner());
    ///     Ok(())
    /// }
    /// ```
    async fn update_flight_path(
        &self,
        request: super::UpdateFlightPathRequest,
    ) -> Result<tonic::Response<super::UpdateResponse>, tonic::Status>;

    /// Returns a [`tonic::Response`] containing a [`BestPathResponse`](super::BestPathResponse)
    /// Takes an [`BestPathRequest`](super::BestPathRequest).
    ///
    /// # Errors
    ///
    /// Returns [`tonic::Status`] with [`Code::Unknown`](tonic::Code::Unknown) if
    /// the server is not ready.
    ///
    /// # Examples
    /// ```
    /// use lib_common::grpc::get_endpoint_from_env;
    /// use lib_common::time::{Utc, Timestamp};
    /// use svc_gis_client_grpc::prelude::*;
    ///
    /// async fn example () -> Result<(), Box<dyn std::error::Error>> {
    ///     let (host, port) = get_endpoint_from_env("SERVER_HOSTNAME", "SERVER_PORT_GRPC");
    ///     let client = GisClient::new_client(&host, port, "gis");
    ///     let time_start: Timestamp = Utc::now().into();
    ///     let time_end: Timestamp = Utc::now().into();
    ///     let request = gis::BestPathRequest {
    ///         origin_identifier: "Kamino".to_string(),
    ///         target_identifier: "Coruscant".to_string(),
    ///         origin_type: 0,
    ///         target_type: 0,
    ///         time_start: Some(time_start),
    ///         time_end: Some(time_end),
    ///         limit: 1
    ///     };
    ///     let response = client.best_path(request).await?;
    ///     println!("RESPONSE={:?}", response.into_inner());
    ///     Ok(())
    /// }
    /// ```
    async fn best_path(
        &self,
        request: super::BestPathRequest,
    ) -> Result<tonic::Response<super::BestPathResponse>, tonic::Status>;

    /// Returns a [`tonic::Response`] containing a [`CheckIntersectionResponse`](super::CheckIntersectionResponse)
    /// Takes an [`CheckIntersectionRequest`](super::CheckIntersectionRequest).
    ///
    /// # Errors
    ///
    /// Returns [`tonic::Status`] with [`Code::Unknown`](tonic::Code::Unknown) if
    /// the server is not ready.
    ///
    /// # Examples
    /// ```
    /// use lib_common::grpc::get_endpoint_from_env;
    /// use lib_common::time::{Utc, Timestamp};
    /// use svc_gis_client_grpc::prelude::*;
    ///
    /// async fn example () -> Result<(), Box<dyn std::error::Error>> {
    ///     let (host, port) = get_endpoint_from_env("SERVER_HOSTNAME", "SERVER_PORT_GRPC");
    ///     let client = GisClient::new_client(&host, port, "gis");
    ///     let time_start: Timestamp = Utc::now().into();
    ///     let time_end: Timestamp = Utc::now().into();
    ///     let request = gis::CheckIntersectionRequest {
    ///         origin_identifier: "Kamino".to_string(),
    ///         target_identifier: "Coruscant".to_string(),
    ///         path: vec![],
    ///         time_start: Some(time_start),
    ///         time_end: Some(time_end)
    ///     };
    ///     let response = client.check_intersection(request).await?;
    ///     println!("RESPONSE={:?}", response.into_inner());
    ///     Ok(())
    /// }
    /// ```
    async fn check_intersection(
        &self,
        request: super::CheckIntersectionRequest,
    ) -> Result<tonic::Response<super::CheckIntersectionResponse>, tonic::Status>;

    /// Returns a [`tonic::Response`] containing a [`GetFlightsResponse`](super::GetFlightsResponse)
    /// Takes an [`GetFlightsRequest`](super::GetFlightsRequest).
    ///
    /// # Errors
    ///
    /// Returns [`tonic::Status`] with [`Code::Unknown`](tonic::Code::Unknown) if
    /// the server is not ready.
    ///
    /// # Examples
    /// ```
    /// use lib_common::grpc::get_endpoint_from_env;
    /// use lib_common::time::{Utc, Timestamp};
    /// use svc_gis_client_grpc::prelude::*;
    ///
    /// async fn example () -> Result<(), Box<dyn std::error::Error>> {
    ///     let (host, port) = get_endpoint_from_env("SERVER_HOSTNAME", "SERVER_PORT_GRPC");
    ///     let client = GisClient::new_client(&host, port, "gis");
    ///     let time_start: Timestamp = Utc::now().into();
    ///     let time_end: Timestamp = Utc::now().into();
    ///     let request = gis::GetFlightsRequest {
    ///         window_min_x: 0.0,
    ///         window_min_y: 0.0,
    ///         window_max_x: 0.0,
    ///         window_max_y: 0.0,
    ///         time_start: Some(time_start),
    ///         time_end: Some(time_end),
    ///     };
    ///     let response = client.get_flights(request).await?;
    ///     println!("RESPONSE={:?}", response.into_inner());
    ///     Ok(())
    /// }
    /// ```
    async fn get_flights(
        &self,
        request: super::GetFlightsRequest,
    ) -> Result<tonic::Response<super::GetFlightsResponse>, tonic::Status>;
}
