import 'package:ease_music_player/services/bridge.service.dart';
import 'package:ease_music_player/services/crash.service.dart';
import 'package:ease_music_player/services/player.service.dart';
import 'package:ease_music_player/services/snackbar.service.dart';

Future<void> initializeServices() async {
  await crashService.initialize();
  bridge.bindFlushSchedule();
  toastService.initialize();
  playerService.bindBridge();
  await bridge.start();
  playerService.initialize();
}

Future<void> destroyServices() async {
  await playerService.destroy();
}
