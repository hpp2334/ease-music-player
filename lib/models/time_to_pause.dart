import 'package:ease_music_player/services/bridge.service.dart';
import 'package:ease_music_player/bridge_generated.dart';
import 'package:flutter/widgets.dart';

class TimeToPauseModel with ChangeNotifier {
  VTimeToPauseState _state = const VTimeToPauseState(
    enabled: false,
    leftHour: 0,
    leftMinute: 0,
  );

  VTimeToPauseState get value => _state;

  TimeToPauseModel() {
    bridge.timeToPauseStream.stream.listen((state) {
      _state = state;
      notifyListeners();
    });
  }
}
