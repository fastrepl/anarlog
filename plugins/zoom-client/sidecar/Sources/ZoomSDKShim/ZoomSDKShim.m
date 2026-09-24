#import "ZoomSDKShim.h"

#import <AppKit/AppKit.h>
#import <ZoomSDK/ZoomSDK.h>

@interface ANLGZoomSession () <ZoomSDKAuthDelegate, ZoomSDKMeetingServiceDelegate,
                               ZoomSDKMeetingActionControllerDelegate>
@property(nonatomic, strong, nullable) ZoomSDKMeetingService *meeting;
@property(nonatomic, strong, nullable) ZoomSDKJoinMeetingElements *joinParameters;
@property(nonatomic, strong) NSMutableSet<NSNumber *> *knownUsers;
@property(nonatomic, strong) NSArray<NSString *> *lastActive;
@property(nonatomic, assign) BOOL initialized;
@property(nonatomic, assign) BOOL ending;
@end

@implementation ANLGZoomSession

- (instancetype)init {
  if ((self = [super init])) {
    _knownUsers = [NSMutableSet set];
    _lastActive = @[];
  }
  return self;
}

- (NSInteger)startWithJWT:(NSString *)jwt
            meetingNumber:(int64_t)meetingNumber
                 passcode:(NSString *)passcode
              displayName:(NSString *)displayName {
  NSAssert(NSThread.isMainThread, @"Zoom operations require the main thread");
  if (self.initialized) return ZoomSDKError_WrongUsage;

  // The SDK's default UI needs a real NSApplication even when we never show our own windows.
  [NSApplication sharedApplication];
  [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];

  ZoomSDKInitParams *params = [ZoomSDKInitParams new];
  params.needCustomizedUI = NO;
  params.enableLog = NO;
  params.zoomDomain = @"zoom.us";
  ZoomSDKError result = [[ZoomSDK sharedSDK] initSDKWithParams:params];
  if (result != ZoomSDKError_Success) return result;
  self.initialized = YES;

  ZoomSDKJoinMeetingElements *context = [ZoomSDKJoinMeetingElements new];
  context.zak = @"";
  context.displayName = displayName;
  context.meetingNumber = meetingNumber;
  context.password = passcode ?: @"";
  context.userType = ZoomSDKUserType_WithoutLogin;
  context.isNoVideo = YES;
  context.isNoAudio = NO;
  self.joinParameters = context;

  ZoomSDKAuthService *auth = [[ZoomSDK sharedSDK] getAuthService];
  auth.delegate = self;
  ZoomSDKAuthContext *authContext = [ZoomSDKAuthContext new];
  authContext.jwtToken = jwt;
  return [auth sdkAuth:authContext];
}

- (void)onZoomSDKAuthReturn:(ZoomSDKAuthError)result {
  if (!NSThread.isMainThread) {
    dispatch_async(dispatch_get_main_queue(), ^{ [self onZoomSDKAuthReturn:result]; });
    return;
  }
  if (result != ZoomSDKAuthError_Success) {
    [self.delegate zoomAuthFailedWithCode:result];
    return;
  }
  self.meeting = [[ZoomSDK sharedSDK] getMeetingService];
  if (!self.meeting) {
    [self.delegate zoomStatusChanged:ANLGZoomStatusFailed code:ZoomSDKError_ServiceFailed];
    return;
  }
  self.meeting.delegate = self;
  [self.meeting getMeetingActionController].delegate = self;
  ZoomSDKError error = [self.meeting joinMeeting:self.joinParameters];
  if (error != ZoomSDKError_Success) {
    [self.delegate zoomStatusChanged:ANLGZoomStatusFailed code:error];
  }
}

- (void)onZoomIdentityExpired {}
- (void)onZoomAuthIdentityExpired {}

- (void)onMeetingStatusChange:(ZoomSDKMeetingStatus)state
                 meetingError:(ZoomSDKMeetingError)error
                    EndReason:(EndMeetingReason)reason {
  if (!NSThread.isMainThread) {
    dispatch_async(dispatch_get_main_queue(), ^{
      [self onMeetingStatusChange:state meetingError:error EndReason:reason];
    });
    return;
  }
  switch (state) {
    case ZoomSDKMeetingStatus_Connecting:
      [self.delegate zoomStatusChanged:ANLGZoomStatusConnecting code:0];
      break;
    case ZoomSDKMeetingStatus_WaitingForHost:
      [self.delegate zoomStatusChanged:ANLGZoomStatusWaitingForHost code:0];
      break;
    case ZoomSDKMeetingStatus_InWaitingRoom:
      [self.delegate zoomStatusChanged:ANLGZoomStatusInWaitingRoom code:0];
      break;
    case ZoomSDKMeetingStatus_InMeeting:
      [self.delegate zoomStatusChanged:ANLGZoomStatusInMeeting code:0];
      [self refreshParticipants];
      break;
    case ZoomSDKMeetingStatus_Disconnecting:
      [self.delegate zoomStatusChanged:ANLGZoomStatusDisconnecting code:0];
      break;
    case ZoomSDKMeetingStatus_Ended:
    case ZoomSDKMeetingStatus_Idle:
      [self.delegate zoomStatusChanged:ANLGZoomStatusEnded code:reason];
      break;
    case ZoomSDKMeetingStatus_Failed:
      [self.delegate zoomStatusChanged:ANLGZoomStatusFailed code:error];
      break;
    default:
      break;
  }
}

- (void)refreshParticipants {
  ZoomSDKMeetingActionController *action = [self.meeting getMeetingActionController];
  NSArray<NSNumber *> *identifiers = [action getParticipantsList] ?: @[];
  NSMutableArray *people = [NSMutableArray array];
  NSMutableArray<NSString *> *talking = [NSMutableArray array];
  NSMutableSet<NSNumber *> *present = [NSMutableSet set];
  for (NSNumber *identifier in identifiers) {
    ZoomSDKUserInfo *user = [action getUserByUserID:identifier.unsignedIntValue];
    if (!user) continue;
    NSString *userID = @([user getUserID]).stringValue;
    [present addObject:identifier];
    [people addObject:@{
      @"id" : userID,
      @"name" : [user getUserName] ?: @"",
      @"isSelf" : @([user isMySelf]),
      @"isTalking" : @([user isTalking]),
    }];
    if ([user isTalking]) [talking addObject:userID];
  }

  NSMutableArray<NSString *> *left = [NSMutableArray array];
  for (NSNumber *known in [self.knownUsers copy]) {
    if (![present containsObject:known]) [left addObject:known.stringValue];
  }
  self.knownUsers = present;

  [self.delegate zoomParticipantsChanged:people];
  if (left.count) [self.delegate zoomParticipantsLeft:left];
  NSArray<NSString *> *sorted =
      [talking sortedArrayUsingSelector:@selector(compare:)];
  if (![sorted isEqualToArray:self.lastActive]) {
    self.lastActive = sorted;
    [self.delegate zoomActiveSpeakersChanged:sorted];
  }
}

- (void)onUserJoin:(NSArray *)array { [self refreshParticipants]; }
- (void)onUserLeft:(NSArray *)array { [self refreshParticipants]; }
- (void)onUserAudioStatusChange:(NSArray *)array { [self refreshParticipants]; }
- (void)onUserActiveAudioChange:(NSArray *)array { [self refreshParticipants]; }
- (void)onUserInfoUpdate:(unsigned int)userID { [self refreshParticipants]; }

- (void)leave {
  if (self.ending) return;
  self.ending = YES;
  [self.meeting leaveMeetingWithCmd:LeaveMeetingCmd_Leave];
}

- (void)shutdown {
  self.meeting.delegate = nil;
  [self.meeting getMeetingActionController].delegate = nil;
  if (self.initialized) {
    [[ZoomSDK sharedSDK] getAuthService].delegate = nil;
    [[ZoomSDK sharedSDK] unInitSDK];
    self.initialized = NO;
  }
  self.meeting = nil;
}

@end
