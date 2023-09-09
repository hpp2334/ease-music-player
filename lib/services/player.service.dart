import 'package:ease_music_player/services/bridge.service.dart';
import 'package:ease_music_player/bridge_generated.dart';
import 'package:ease_music_player/services/snackbar.service.dart';
import 'package:flutter/foundation.dart';
import 'package:just_audio/just_audio.dart';
import 'package:audio_service/audio_service.dart';

class PlayerService extends BaseAudioHandler with SeekHandler {
  final AudioPlayer _player = AudioPlayer();

  PlayerService() {
    _player.playbackEventStream.map(_transformEvent).pipe(playbackState);
  }

  Future<void> destroy() async {
    await _player.dispose();
  }

  executeWithErrorHandled(String scope, Future<void> Function() f) async {
    try {
      await f();
    } on PlayerException catch (e) {
      _player.pause();
      toastService.showError(e.message ?? "Player unknown error");

      if (kDebugMode) {
        print("[$scope] PlayerException: ${e.message}");
      }
    } on PlayerInterruptedException catch (e) {
      if (kDebugMode) {
        print("[$scope] Connection aborted: ${e.message}");
      }
    } catch (e) {
      if (kDebugMode) {
        print('[$scope] An error occurred: $e');
      }
    }
  }

  void bindBridge() {
    bridge.getRawBinding().initBindResumeMusic().listen((event) {
      executeWithErrorHandled("RESUME_MUSIC", () => _player.play());
    });
    bridge.getRawBinding().initBindPauseMusic().listen((event) {
      _player.pause();
    });
    bridge.getRawBinding().initBindStopMusic().listen((event) {
      _player.stop();
    });
    bridge.getRawBinding().initSeekMusic().listen((arg) {
      executeWithErrorHandled(
          "SEEK_MUSIC", () => _player.seek(Duration(milliseconds: arg)));
    });
    bridge.getRawBinding().setMusicUrl().listen((arg) async {
      if (kDebugMode) {
        print('SET_MUSIC_URL $arg');
      }
      executeWithErrorHandled("SET_MUSIC_URL", () async {
        await _player.setUrl(arg);
      });
    });
    bridge.currentMusicForNotificationStream.stream.listen((state) {
      final id = state.id;
      if (id == null) {
        return;
      }
      mediaItem.add(MediaItem(
        id: id.field0.toString(),
        title: state.title,
        duration: Duration(milliseconds: state.totalDurationMs),
      ));
    });
  }

  void initialize() async {
    await AudioService.init(
      builder: () => this,
      config: const AudioServiceConfig(
        androidNotificationChannelId: 'com.ryanheise.myapp.channel.audio',
        androidNotificationChannelName: 'Audio playback',
        androidNotificationOngoing: true,
      ),
    );

    _player.positionStream.listen((event) {
      bridge.scope((api) => api.setCurrentMusicPositionForPlayerInternal(
            arg: _player.position.inMilliseconds,
          ));
    });
    _player.durationStream.listen((event) {
      if (event != null && event.inMilliseconds > 0) {
        bridge.scope((api) =>
            api.updateCurrentMusicTotalDurationForPlayerInternal(
                arg: event.inMilliseconds));
      }
    });
    _player.playingStream.listen((arg) {
      bridge.scope(
          (api) => api.updateCurrentMusicPlayingForPlayerInternal(arg: arg));
    });
    _player.playerStateStream.listen((event) {
      if (kDebugMode) {
        print(
            'playerStateStream event.processingState ${event.processingState}');
      }
      if (event.processingState == ProcessingState.completed) {
        bridge.scope((api) => api.handlePlayMusicEventForPlayerInternal(
              arg: PlayMusicEventType.Complete,
            ));
      } else if (event.processingState == ProcessingState.buffering ||
          event.processingState == ProcessingState.loading) {
        bridge.scope((api) => api.handlePlayMusicEventForPlayerInternal(
            arg: PlayMusicEventType.Loading));
      } else if (event.processingState == ProcessingState.ready ||
          event.processingState == ProcessingState.idle) {
        bridge.scope((api) => api.handlePlayMusicEventForPlayerInternal(
            arg: PlayMusicEventType.Loaded));
      }
    });
  }

  @override
  Future<void> play() => _player.play();

  startPlay(MusicId musicId) {
    bridge.scope((api) => api.playMusic(arg: musicId));
  }

  @override
  Future<void> pause() async {
    bridge.scope((api) => api.pauseMusic());
  }

  resume() {
    bridge.scope((api) => api.resumeMusic());
  }

  @override
  Future<void> stop() async {
    bridge.scope((api) => api.stopMusic());
  }

  @override
  Future<void> seek(Duration position) async {
    bridge.scope((api) =>
        api.seekMusic(arg: ArgSeekMusic(duration: position.inSeconds)));
  }

  @override
  Future<void> skipToNext() async {
    bridge.scope((api) => api.playNextMusic());
  }

  @override
  Future<void> skipToPrevious() async {
    bridge.scope((api) => api.playPreviousMusic());
  }

  updateMusicPlayModeToNext() {
    bridge.scope((api) => api.updateMusicPlaymodeToNext());
  }

  PlaybackState _transformEvent(PlaybackEvent event) {
    return PlaybackState(
      controls: [
        MediaControl.rewind,
        if (_player.playing) MediaControl.pause else MediaControl.play,
        MediaControl.stop,
        MediaControl.fastForward,
      ],
      systemActions: const {
        MediaAction.seek,
        MediaAction.seekForward,
        MediaAction.seekBackward,
      },
      androidCompactActionIndices: const [0, 1, 3],
      processingState: const {
        ProcessingState.idle: AudioProcessingState.idle,
        ProcessingState.loading: AudioProcessingState.loading,
        ProcessingState.buffering: AudioProcessingState.buffering,
        ProcessingState.ready: AudioProcessingState.ready,
        ProcessingState.completed: AudioProcessingState.completed,
      }[_player.processingState]!,
      playing: _player.playing,
      updatePosition: _player.position,
      bufferedPosition: _player.bufferedPosition,
      speed: _player.speed,
      queueIndex: event.currentIndex,
    );
  }
}

final playerService = PlayerService();
