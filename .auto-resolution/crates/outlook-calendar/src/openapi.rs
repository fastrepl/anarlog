use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(components(schemas(
    crate::Attendee,
    crate::AttendeeType,
    crate::BodyType,
    crate::Calendar,
    crate::CalendarColor,
    crate::CreateEventBody,
    crate::DateTimeTimeZone,
    crate::DayOfWeek,
    crate::EmailAddress,
    crate::Event,
    crate::EventShowAs,
    crate::EventType,
    crate::Importance,
    crate::ItemBody,
    crate::ListCalendarsResponse,
    crate::ListEventsResponse,
    crate::Location,
    crate::LocationType,
    crate::OnlineMeetingInfo,
    crate::OnlineMeetingProviderType,
    crate::OutlookGeoCoordinates,
    crate::PatternedRecurrence,
    crate::PhysicalAddress,
    crate::Recipient,
    crate::RecurrencePattern,
    crate::RecurrencePatternType,
    crate::RecurrenceRange,
    crate::RecurrenceRangeType,
    crate::ResponseStatus,
    crate::ResponseType,
    crate::Sensitivity,
    crate::WeekIndex,
)))]
struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
