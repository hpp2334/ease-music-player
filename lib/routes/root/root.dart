import 'package:ease_music_player/services/bridge.service.dart';
import 'package:ease_music_player/bridge_generated.dart';
import 'package:ease_music_player/models/current_router.dart';
import 'package:ease_music_player/routes/dashboard/stab_dashboard.dart';
import 'package:ease_music_player/routes/playlist/stab_list.dart';
import 'package:ease_music_player/routes/setting/stab_setting.dart';
import 'package:ease_music_player/widgets/screen_container.dart';
import 'package:flutter/widgets.dart';
import 'package:provider/provider.dart';

class RootScreenPage extends StatefulWidget {
  const RootScreenPage({super.key});

  @override
  State<RootScreenPage> createState() => _RootScreenPageState();
}

class _RootScreenPageState extends State<RootScreenPage> {
  PageController controller = PageController();

  @override
  void initState() {
    super.initState();

    controller.addListener(() {
      if (controller.page == 0) {
        bridge.scope(
            (api) => api.updateRootSubkey(arg: RootRouteSubKey.Playlist));
      } else if (controller.page == 1) {
        bridge.scope(
            (api) => api.updateRootSubkey(arg: RootRouteSubKey.Dashboard));
      } else if (controller.page == 2) {
        bridge
            .scope((api) => api.updateRootSubkey(arg: RootRouteSubKey.Setting));
      }
    });
  }

  @override
  void dispose() {
    controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final state = context.watch<RootSubKeyModel>().value;
    final subkey = state.subkey;

    return EaseScreenContainer(
      subScreenKey: subkey,
      onTapNav: (subKey) {
        switch (subKey) {
          case RootRouteSubKey.Playlist:
            {
              controller.jumpToPage(0);
              break;
            }
          case RootRouteSubKey.Dashboard:
            {
              controller.jumpToPage(1);
              break;
            }
          case RootRouteSubKey.Setting:
            {
              controller.jumpToPage(2);
              break;
            }
        }
      },
      child: PageView(
        controller: controller,
        children: const [
          PlaylistListStab(),
          DashboardStab(),
          SettingStab(),
        ],
      ),
    );
  }
}
