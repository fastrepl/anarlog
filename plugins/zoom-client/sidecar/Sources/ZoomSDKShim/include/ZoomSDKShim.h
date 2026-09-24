#import <Foundation/Foundation.h>

NS_ASSUME_NONNULL_BEGIN

typedef NS_ENUM(NSInteger, ANLGZoomStatus) {
  ANLGZoomStatusConnecting,
  ANLGZoomStatusWaitingForHost,
  ANLGZoomStatusInWaitingRoom,
  ANLGZoomStatusInMeeting,
  ANLGZoomStatusDisconnecting,
  ANLGZoomStatusEnded,
  ANLGZoomStatusFailed,
};

@protocol ANLGZoomSessionDelegate <NSObject>
- (void)zoomAuthFailedWithCode:(NSInteger)code;
- (void)zoomStatusChanged:(ANLGZoomStatus)status code:(NSInteger)code;
/// Each entry: `id` (NSString), `name` (NSString), `isSelf`, `isTalking` (NSNumber BOOL).
- (void)zoomParticipantsChanged:(NSArray<NSDictionary<NSString *, id> *> *)participants;
- (void)zoomParticipantsLeft:(NSArray<NSString *> *)participantIDs;
- (void)zoomActiveSpeakersChanged:(NSArray<NSString *> *)participantIDs;
@end

/// Thin main-thread wrapper over the Zoom Meeting SDK default-UI join flow. Owns exactly
/// one SDK lifetime per process.
@interface ANLGZoomSession : NSObject
@property(nonatomic, weak, nullable) id<ANLGZoomSessionDelegate> delegate;
- (NSInteger)startWithJWT:(NSString *)jwt
            meetingNumber:(int64_t)meetingNumber
                 passcode:(nullable NSString *)passcode
              displayName:(NSString *)displayName;
- (void)leave;
- (void)shutdown;
@end

NS_ASSUME_NONNULL_END
