import 'dart:async';
import 'dart:io';

import 'package:ease_music_player/global.dart';
import 'package:ease_music_player/widgets/button.dart';
import 'package:ease_music_player/widgets/screen_container.dart';
import 'package:flutter/scheduler.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:path_provider/path_provider.dart';

class CrashScreenPage extends StatefulWidget {
  const CrashScreenPage({super.key});

  @override
  State<CrashScreenPage> createState() => _CrashScreenPageState();
}

class _CrashScreenPageState extends State<CrashScreenPage> {
  String log = "";
  bool copied = false;

  @override
  void initState() {
    super.initState();

    SchedulerBinding.instance.addPostFrameCallback((timeStamp) async {
      final Directory documentDir = await getApplicationDocumentsDirectory();
      final String documentDirPath = documentDir.path.endsWith("/")
          ? documentDir.path
          : "${documentDir.path}/";
      final String data =
          File("${documentDirPath}latest.log").readAsStringSync();
      setState(() {
        log = data;
      });
    });
  }

  @override
  Widget build(BuildContext context) {
    return EaseScreenContainer(
      hidePlayer: true,
      child: Container(
        padding: const EdgeInsets.all(24),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Text(
              "Oops...",
              style: TextStyle(
                fontSize: 32,
                color: EaseColors.primaryText,
              ),
            ),
            const Text(
              "The App encountered an unrecoverable error. You can copy the log to github and make an issue to help us improve the App.",
              style: TextStyle(
                fontSize: 14,
                color: EaseColors.primaryText,
              ),
            ),
            const SizedBox(height: 12),
            Expanded(
              child: Container(
                padding: const EdgeInsets.all(12),
                decoration: BoxDecoration(
                  borderRadius: BorderRadius.circular(10),
                  color: EaseColors.light,
                ),
                child: SingleChildScrollView(
                  child: Text(
                    log,
                    style: const TextStyle(
                      fontSize: 12,
                      color: EaseColors.primaryText,
                    ),
                  ),
                ),
              ),
            ),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                EaseTextButton(
                  text: copied ? "COPIED" : "COPY",
                  onPressed: () {
                    Clipboard.setData(ClipboardData(text: log));
                    setState(() {
                      copied = true;
                    });
                    Timer(
                      const Duration(seconds: 2),
                      () {
                        setState(() {
                          copied = false;
                        });
                      },
                    );
                  },
                ),
                EaseTextButton(
                  text: "EXIT",
                  onPressed: () {
                    exit(1);
                  },
                ),
              ],
            )
          ],
        ),
      ),
    );
  }
}
