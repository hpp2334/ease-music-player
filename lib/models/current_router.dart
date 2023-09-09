import 'package:ease_music_player/services/bridge.service.dart';
import 'package:ease_music_player/bridge_generated.dart';
import 'package:flutter/widgets.dart';

class RootSubKeyModel with ChangeNotifier {
  VRootSubKeyState _state = const VRootSubKeyState(
    subkey: RootRouteSubKey.Playlist,
  );

  VRootSubKeyState get value => _state;

  RootSubKeyModel() {
    bridge.rootSubKeyStream.stream.listen((state) {
      _state = state;
      notifyListeners();
    });
  }
}
