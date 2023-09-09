import 'package:ease_music_player/services/bridge.service.dart';
import 'package:ease_music_player/bridge_generated.dart';
import 'package:flutter/widgets.dart';

class CurrentMusicModel with ChangeNotifier {
  VCurrentMusicState _state = const VCurrentMusicState(
    title: '',
    currentDuration: '',
    totalDuration: '',
    currentDurationMs: 0,
    totalDurationMs: 0,
    canChangePosition: false,
    canPlayNext: false,
    canPlayPrevious: false,
    playMode: PlayMode.Single,
    playing: false,
    lyricIndex: -1,
    previousCover: 0,
    nextCover: 0,
    cover: 0,
    loading: true,
  );

  VCurrentMusicState get value => _state;

  CurrentMusicModel() {
    bridge.currentMusicStream.stream.listen((state) {
      _state = state;
      notifyListeners();
    });
  }
}
