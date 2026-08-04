use crate::packet_utils::Buf;
use crate::{Bot, Compression};
use rand::Rng;

/// Clientbound Keep Alive (play)
/// https://minecraft.wiki/w/Java_Edition_protocol/Packets#Clientbound_Keep_Alive_(play)
pub fn process_keep_alive_packet(buffer: &mut Buf, bot: &mut Bot, compression: &mut Compression) {
    bot.send_packet(write_keep_alive_packet(buffer.read_u64()), compression);
}

/// Disconnect (login/config/play)
/// https://minecraft.wiki/w/Java_Edition_protocol/Packets#Disconnect_(login)
///
/// The message field is an NBT-encoded Component (confirmed via decompiling
/// DisconnectPacket.java), not a sized string -- don't attempt to parse it as one, that would
/// misread the NBT and risk panicking on a bogus length. The framing loop already advances past
/// this packet by its declared length regardless of what we read here, so just flag the kick.
pub fn process_kick(_buffer: &mut Buf, bot: &mut Bot, _compression: &mut Compression) {
    println!("bot \"{}\" was kicked (see server log/NBT reason, not decoded here)", bot.name);
    bot.kicked = true;
}

/// Login (play)
/// https://minecraft.wiki/w/Java_Edition_protocol/Packets#Login_(play)
pub fn process_join_game(buffer: &mut Buf, bot: &mut Bot, compression: &mut Compression) {
    bot.entity_id = buffer.read_u32();

    // Real clients send Player Loaded once their loading screen finishes; the server uses it to
    // fire PlayerLoadedEvent (confirmed via decompiling PlayerLoadedListener.java, wire ID 0x2C
    // in CLIENT_PLAY, confirmed via decompiling PacketVanilla -- ClientPlayerLoadedPacket is an
    // empty record, no payload). This bot never renders anything, so there's nothing to wait on --
    // send it immediately. Without it, server-side code that hooks PlayerLoadedEvent (e.g. this
    // server's Resident.create()) never runs for bots, silently breaking anything gated on it.
    bot.send_packet(write_player_loaded(), compression);
}

/// Player Loaded (serverbound)
pub fn write_player_loaded() -> Buf {
    // ClientPlayerLoadedPacket
    let mut buf = Buf::new();
    buf.write_packet_id(0x2C);

    buf
}

/// Synchronize Player Position
/// https://minecraft.wiki/w/Java_Edition_protocol/Packets#Synchronize_Player_Position
pub fn process_teleport(buffer: &mut Buf, bot: &mut Bot, compression: &mut Compression) {
    let id = buffer.read_var_u32().0;
    let x = buffer.read_f64();
    let y = buffer.read_f64();
    let z = buffer.read_f64();
    let _yaw = buffer.read_f32();
    let _pitch = buffer.read_f32();
    let flags = buffer.read_byte();
    if flags & 0b10000 == 0 {
        bot.x = x;
    } else {
        bot.x += x;
    }
    if flags & 0b01000 == 0 {
        bot.y = y;
    } else {
        bot.y += y;
    }
    if flags & 0b00100 == 0 {
        bot.z = z;
    } else {
        bot.z += z;
    }
    bot.send_packet(write_tele_confirm(id), compression);

    // The server spawns every bot at the same world coordinate, so a large swarm piles up on top
    // of itself for the whole test -- entity tracking/visibility work scales with local density,
    // not bot count, so a dense pile is a much heavier (and less representative) load than the
    // same bot count spread across the map. Fan each bot out over a wide area once, right after
    // its first teleport, instead of leaving it to the slow +/-0.5 random walk below.
    if !bot.teleported {
        let mut rng = rand::thread_rng();
        bot.x += rng.gen_range(-150.0..150.0);
        bot.z += rng.gen_range(-150.0..150.0);
        bot.send_packet(write_current_pos(bot), compression);
    }

    bot.teleported = true;
    println!("{x}, {y}, {z}");
}

/// System Chat Message (clientbound) -- TEMPORARY war-flag-verification probe.
/// https://minecraft.wiki/w/Java_Edition_protocol/Packets#System_Chat_Message
///
/// The `message` field is an NBT-encoded Component (confirmed by decompiling
/// SystemChatPacket.class -- same NBT-Component encoding as DisconnectPacket, see process_kick
/// above), not a sized string, so it isn't parsed properly here. This is a throwaway diagnostic
/// to read the server's flag-attack success ("is attacking...") or failure (Message.error(...))
/// text back from a real connected bot -- good enough to just scan the raw payload bytes for
/// printable-ASCII runs rather than write a real NBT/Component decoder for a handler that gets
/// deleted once verification is done.
pub fn process_system_chat(buffer: &mut Buf, bot: &mut Bot, _compression: &mut Compression) {
    let remaining = buffer.get_writer_index() - buffer.get_reader_index();
    let bytes = buffer.read_bytes(remaining);

    let mut current = String::new();
    let mut runs: Vec<String> = Vec::new();
    for &b in bytes {
        if b.is_ascii_graphic() || b == b' ' {
            current.push(b as char);
        } else {
            if current.len() >= 3 {
                runs.push(current.clone());
            }
            current.clear();
        }
    }
    if current.len() >= 3 {
        runs.push(current);
    }
    println!("[SystemChat -> {}] {:?}", bot.name, runs);
}

/// Chat Message
/// https://minecraft.wiki/w/Java_Edition_protocol/Packets#Chat_Message
///
/// Packet IDs below were confirmed against the actual PLAY-state client packet registry of the
/// exact Minestom build being tested (net.minestom.server.network.packet.PacketVanilla.CLIENT_PLAY),
/// not assumed from upstream's default protocol version -- they had drifted from upstream's
/// 0x08/0x3C/0x29/0x34/0x1b/0x1E for this build (0x09/0x3F/0x2A/0x35/0x1C/0x1F), and Minestom was
/// silently no-oping the malformed packets rather than kicking, so this had gone unnoticed.
pub fn write_chat_message(message: &str) -> Buf {
    // ClientChatMessagePacket
    let mut buf = Buf::new();
    buf.write_packet_id(0x09);

    buf.write_sized_str(message);

    // 1.19 signing fields
    buf.write_u64(0); // timestamp
    buf.write_u64(0); // salt
    buf.write_bool(false); // has signature
    buf.write_var_u32(0); // count
    buf.write_bytes(&[0; 3]); // bitset
    buf.write_var_u32(0); // signature count

    buf
}

/// Swing Arm
/// https://minecraft.wiki/w/Java_Edition_protocol/Packets#Swing_Arm
pub fn write_animation(off_hand: bool) -> Buf {
    // ClientAnimationPacket
    let mut buf = Buf::new();
    buf.write_packet_id(0x3F);
    buf.write_var_u32(if off_hand { 1 } else { 0 });

    buf
}

/// Player Command (serverbound) -- sneak/sprint/etc, NOT digging (see write_player_action below,
/// which is a different packet: ClientPlayerActionPacket).
/// https://minecraft.wiki/w/Java_Edition_protocol/Packets#Player_Command
pub fn write_entity_action(entity_id: u32, action_id: u32, jump_boost: u32) -> Buf {
    // ClientEntityActionPacket
    let mut buf = Buf::new();
    buf.write_packet_id(0x2A);

    buf.write_var_u32(entity_id);
    buf.write_var_u32(action_id);
    buf.write_var_u32(jump_boost);

    buf
}

/// Player Action (serverbound) -- digging/breaking blocks.
/// https://minecraft.wiki/w/Java_Edition_protocol/Packets#Player_Action
///
/// status: 0=started digging, 1=cancelled digging, 2=finished digging (matches
/// ClientPlayerActionPacket$Status's enum ordinal on the server).
/// face: 0=bottom, 1=top, 2=north, 3=south, 4=west, 5=east (matches BlockFace's ordinal).
pub fn write_player_action(status: u32, x: i32, y: i32, z: i32, face: u32, sequence: u32) -> Buf {
    // ClientPlayerActionPacket
    let mut buf = Buf::new();
    buf.write_packet_id(0x29);

    buf.write_var_u32(status);
    buf.write_block_position(x, y, z);
    buf.write_var_u32(face);
    buf.write_var_u32(sequence);

    buf
}

/// Player Action (serverbound) -- use item on block / place block.
/// https://minecraft.wiki/w/Java_Edition_protocol/Packets#Use_Item_On
///
/// Confirmed against the same decompiled PacketVanilla registry as the header comment above:
/// ClientPlayerBlockPlacementPacket is ID 0x42 on this build.
///
/// hand: 0=main hand, 1=off hand (PlayerHand ordinal). face: 0=bottom, 1=top, 2=north, 3=south,
/// 4=west, 5=east (BlockFace ordinal, same as write_player_action). Places whatever item is
/// currently in the given hand's selected slot against the face of the block at (x,y,z) --
/// the block type placed is NOT part of this packet, it's determined server-side from the
/// player's held item.
pub fn write_block_place(
    hand: u32,
    x: i32,
    y: i32,
    z: i32,
    face: u32,
    cursor_x: f32,
    cursor_y: f32,
    cursor_z: f32,
    sequence: u32,
) -> Buf {
    // ClientPlayerBlockPlacementPacket
    let mut buf = Buf::new();
    buf.write_packet_id(0x42);

    buf.write_var_u32(hand);
    buf.write_block_position(x, y, z);
    buf.write_var_u32(face);
    buf.write_f32(cursor_x);
    buf.write_f32(cursor_y);
    buf.write_f32(cursor_z);
    buf.write_bool(false); // insideBlock
    buf.write_bool(false); // hitWorldBorder
    buf.write_var_u32(sequence);

    buf
}

/// Set Held Item (serverbound)
/// https://minecraft.wiki/w/Java_Edition_protocol/Packets#Set_Held_Item_(serverbound)
pub fn write_held_slot(slot: u16) -> Buf {
    // ClientHeldItemChangePacket
    let mut buf = Buf::new();
    buf.write_packet_id(0x35);

    buf.write_u16(slot);

    buf
}

/// Confirm Teleportation
/// https://minecraft.wiki/w/Java_Edition_protocol/Packets#Confirm_Teleportation
pub fn write_tele_confirm(id: u32) -> Buf {
    // ClientTeleportConfirmPacket
    let mut buf = Buf::new();
    buf.write_packet_id(0x00);

    buf.write_var_u32(id);

    buf
}

/// Serverbound Keep Alive (play)
/// https://minecraft.wiki/w/Java_Edition_protocol/Packets#Serverbound_Keep_Alive_(play)
pub fn write_keep_alive_packet(id: u64) -> Buf {
    // ClientKeepAlivePacket
    let mut buf = Buf::new();
    buf.write_packet_id(0x1C);

    buf.write_u64(id);

    buf
}

pub fn write_current_pos(bot: &Bot) -> Buf {
    write_pos(bot.x, bot.y, bot.z, 0.0, 0.0)
}

/// Set Player Position and Rotation
/// https://minecraft.wiki/w/Java_Edition_protocol/Packets#Set_Player_Position_and_Rotation
pub fn write_pos(x: f64, y: f64, z: f64, yaw: f32, pitch: f32) -> Buf {
    // ClientPlayerPositionAndRotationPacket
    let mut buf = Buf::new();
    buf.write_packet_id(0x1F);

    buf.write_f64(x);
    buf.write_f64(y);
    buf.write_f64(z);

    buf.write_f32(yaw);
    buf.write_f32(pitch);

    buf.write_bool(false);

    buf
}
