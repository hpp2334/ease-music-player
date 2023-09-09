import 'package:ease_music_player/global.dart';
import 'package:ease_music_player/models/create_playlist.dart';
import 'package:ease_music_player/models/current_music.dart';
import 'package:ease_music_player/models/current_music_lyric.dart';
import 'package:ease_music_player/models/current_playlist.dart';
import 'package:ease_music_player/models/current_router.dart';
import 'package:ease_music_player/models/current_storage_entries.dart';
import 'package:ease_music_player/models/edit_playlist.dart';
import 'package:ease_music_player/models/edit_storage.dart';
import 'package:ease_music_player/models/playlist_list.dart';
import 'package:ease_music_player/models/storage_list.dart';
import 'package:ease_music_player/models/time_to_pause.dart';
import 'package:ease_music_player/services/router.service.dart';
import 'package:ease_music_player/services/services.dart';
import 'package:ease_music_player/services/snackbar.service.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

void main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await initializeServices();

  runApp(MultiProvider(
    providers: [
      ChangeNotifierProvider(create: (_) => StorageListModel()),
      ChangeNotifierProvider(create: (_) => CurrentStorageEntriesModel()),
      ChangeNotifierProvider(create: (_) => PlaylistListModel()),
      ChangeNotifierProvider(create: (_) => CurrentPlaylistModel()),
      ChangeNotifierProvider(create: (_) => CurrentMusicModel()),
      ChangeNotifierProvider(create: (_) => TimeToPauseModel()),
      ChangeNotifierProvider(create: (_) => CurrentMusicLyricModel()),
      ChangeNotifierProvider(create: (_) => RootSubKeyModel()),
      ChangeNotifierProvider(create: (_) => EditPlaylistModel()),
      ChangeNotifierProvider(create: (_) => CreatePlaylistModel()),
      ChangeNotifierProvider(create: (_) => EditStorageModel()),
    ],
    child: const MyApp(),
  ));
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Ease Music Player',
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: EaseColors.primary),
        useMaterial3: true,
      ),
      navigatorKey: routerService.navigatorKey,
      scaffoldMessengerKey: toastService.snackbarGlobalKey,
      onGenerateRoute: generateRoutes,
    );
  }
}
