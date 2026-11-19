#import "AudioEngineGuard.h"

#if !__has_feature(objc_arc)
#error "AudioEngineGuard needs ARC"
#endif

NSErrorDomain const AnarlogAudioEngineErrorDomain = @"AnarlogAudioEngine";

static BOOL AnarlogFail(NSException *exception, NSError **error) {
  if (error) {
    NSString *reason = exception.reason ?: exception.name;
    *error = [NSError errorWithDomain:AnarlogAudioEngineErrorDomain
                                 code:1
                             userInfo:@{
                               NSLocalizedDescriptionKey : reason,
                               @"exceptionName" : exception.name,
                             }];
  }
  return NO;
}

@implementation AVAudioEngine (AnarlogGuarded)

- (AVAudioInputNode *)anlg_inputNodeWithError:(NSError **)error {
  @try {
    return self.inputNode;
  } @catch (NSException *exception) {
    AnarlogFail(exception, error);
    return nil;
  }
}

- (BOOL)anlg_attachNode:(AVAudioNode *)node error:(NSError **)error {
  @try {
    [self attachNode:node];
    return YES;
  } @catch (NSException *exception) {
    return AnarlogFail(exception, error);
  }
}

- (BOOL)anlg_detachNode:(AVAudioNode *)node error:(NSError **)error {
  @try {
    [self detachNode:node];
    return YES;
  } @catch (NSException *exception) {
    return AnarlogFail(exception, error);
  }
}

- (BOOL)anlg_connect:(AVAudioNode *)node
                  to:(AVAudioNode *)target
              format:(AVAudioFormat *)format
               error:(NSError **)error {
  @try {
    [self connect:node to:target format:format];
    return YES;
  } @catch (NSException *exception) {
    return AnarlogFail(exception, error);
  }
}

- (BOOL)anlg_startWithError:(NSError **)error {
  @try {
    return [self startAndReturnError:error];
  } @catch (NSException *exception) {
    return AnarlogFail(exception, error);
  }
}

- (BOOL)anlg_stopWithError:(NSError **)error {
  @try {
    [self stop];
    return YES;
  } @catch (NSException *exception) {
    return AnarlogFail(exception, error);
  }
}

@end

@implementation AVAudioNode (AnarlogGuarded)

- (AVAudioFormat *)anlg_outputFormatForBus:(AVAudioNodeBus)bus error:(NSError **)error {
  @try {
    return [self outputFormatForBus:bus];
  } @catch (NSException *exception) {
    AnarlogFail(exception, error);
    return nil;
  }
}

- (AVAudioFormat *)anlg_inputFormatForBus:(AVAudioNodeBus)bus error:(NSError **)error {
  @try {
    return [self inputFormatForBus:bus];
  } @catch (NSException *exception) {
    AnarlogFail(exception, error);
    return nil;
  }
}

- (BOOL)anlg_installTapOnBus:(AVAudioNodeBus)bus
                  bufferSize:(AVAudioFrameCount)bufferSize
                      format:(AVAudioFormat *)format
                       block:(AVAudioNodeTapBlock)block
                       error:(NSError **)error {
  @try {
    [self installTapOnBus:bus bufferSize:bufferSize format:format block:block];
    return YES;
  } @catch (NSException *exception) {
    return AnarlogFail(exception, error);
  }
}

- (BOOL)anlg_removeTapOnBus:(AVAudioNodeBus)bus error:(NSError **)error {
  @try {
    [self removeTapOnBus:bus];
    return YES;
  } @catch (NSException *exception) {
    return AnarlogFail(exception, error);
  }
}

@end

@implementation AVAudioInputNode (AnarlogGuarded)

- (BOOL)anlg_setVoiceProcessing:(BOOL)enabled
                advancedDucking:(BOOL)advancedDucking
                          error:(NSError **)error {
  @try {
    if (![self setVoiceProcessingEnabled:enabled error:error]) return NO;
    if (enabled) {
      self.voiceProcessingAGCEnabled = NO;
      AVAudioVoiceProcessingOtherAudioDuckingConfiguration ducking = {
          .enableAdvancedDucking = advancedDucking,
          .duckingLevel = AVAudioVoiceProcessingOtherAudioDuckingLevelMin,
      };
      self.voiceProcessingOtherAudioDuckingConfiguration = ducking;
    }
    return YES;
  } @catch (NSException *exception) {
    return AnarlogFail(exception, error);
  }
}

- (BOOL)anlg_readVoiceProcessingDucking:
            (AVAudioVoiceProcessingOtherAudioDuckingConfiguration *)ducking
                                    agc:(BOOL *)agc
                                  error:(NSError **)error {
  @try {
    *ducking = self.voiceProcessingOtherAudioDuckingConfiguration;
    *agc = self.voiceProcessingAGCEnabled;
    return YES;
  } @catch (NSException *exception) {
    return AnarlogFail(exception, error);
  }
}

@end
