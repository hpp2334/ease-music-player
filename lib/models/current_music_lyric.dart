import 'package:ease_music_player/services/bridge.service.dart';
import 'package:ease_music_player/bridge_generated.dart';
import 'package:flutter/widgets.dart';

class CurrentMusicLyricModel with ChangeNotifier {
  VCurrentMusicLyricState _state = const VCurrentMusicLyricState(
    lyricLines: [],
    loadState: LyricLoadState.Missing,
  );

  VCurrentMusicLyricState get value => _state;

  CurrentMusicLyricModel() {
    bridge.currentMusicLyricStream.stream.listen((state) {
      _state = state;
      notifyListeners();
    });
  }
}
