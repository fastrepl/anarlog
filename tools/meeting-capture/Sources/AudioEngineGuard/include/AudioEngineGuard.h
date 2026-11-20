#import <AVFoundation/AVFoundation.h>

NS_ASSUME_NONNULL_BEGIN

/// AVAudioEngine reports many failures by raising an Objective-C exception, which Swift cannot
/// catch: the process aborts. A device change can make any of these calls raise, for example a
/// Bluetooth headset switching profiles between reading the input format and installing the tap.
/// Each call here catches the exception and returns it as an NSError, so Swift sees a throw.
///
/// Each call has its own method instead of one block-taking wrapper: an exception must never
/// unwind through Swift frames, which skips their releases and cleanup.
FOUNDATION_EXPORT NSErrorDomain const AnarlogAudioEngineErrorDomain;

@interface AVAudioEngine (AnarlogGuarded)
- (nullable AVAudioInputNode *)anlg_inputNodeWithError:(NSError **)error
    NS_SWIFT_NAME(guardedInputNode());
- (BOOL)anlg_attachNode:(AVAudioNode *)node error:(NSError **)error
    NS_SWIFT_NAME(guardedAttach(_:));
- (BOOL)anlg_detachNode:(AVAudioNode *)node error:(NSError **)error
    NS_SWIFT_NAME(guardedDetach(_:));
- (BOOL)anlg_connect:(AVAudioNode *)node
                  to:(AVAudioNode *)target
              format:(nullable AVAudioFormat *)format
               error:(NSError **)error NS_SWIFT_NAME(guardedConnect(_:to:format:));
- (BOOL)anlg_startWithError:(NSError **)error NS_SWIFT_NAME(guardedStart());
- (BOOL)anlg_stopWithError:(NSError **)error NS_SWIFT_NAME(guardedStop());
@end

@interface AVAudioNode (AnarlogGuarded)
- (nullable AVAudioFormat *)anlg_outputFormatForBus:(AVAudioNodeBus)bus
                                              error:(NSError **)error
    NS_SWIFT_NAME(guardedOutputFormat(forBus:));
- (nullable AVAudioFormat *)anlg_inputFormatForBus:(AVAudioNodeBus)bus
                                             error:(NSError **)error
    NS_SWIFT_NAME(guardedInputFormat(forBus:));
- (BOOL)anlg_installTapOnBus:(AVAudioNodeBus)bus
                  bufferSize:(AVAudioFrameCount)bufferSize
                      format:(nullable AVAudioFormat *)format
                       block:(AVAudioNodeTapBlock)block
                       error:(NSError **)error
    NS_SWIFT_NAME(guardedInstallTap(onBus:bufferSize:format:block:));
- (BOOL)anlg_removeTapOnBus:(AVAudioNodeBus)bus error:(NSError **)error
    NS_SWIFT_NAME(guardedRemoveTap(onBus:));
@end

@interface AVAudioInputNode (AnarlogGuarded)
/// Turns voice processing on or off. On, it also turns off its gain control and keeps its ducking
/// of other audio to the minimum; `advancedDucking` lets that ducking follow voice activity.
- (BOOL)anlg_setVoiceProcessing:(BOOL)enabled
                advancedDucking:(BOOL)advancedDucking
                          error:(NSError **)error
    NS_SWIFT_NAME(guardedSetVoiceProcessing(_:advancedDucking:));
/// Reads back the ducking and gain control the voice processing unit applies, for the journal.
- (BOOL)anlg_readVoiceProcessingDucking:
            (AVAudioVoiceProcessingOtherAudioDuckingConfiguration *)ducking
                                    agc:(BOOL *)agc
                                  error:(NSError **)error
    NS_SWIFT_NAME(guardedReadVoiceProcessing(ducking:agc:));
@end

NS_ASSUME_NONNULL_END
