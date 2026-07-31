use crate::{packet_utils::Buf, Bot, Compression, ProtocolState};

/// https://minecraft.wiki/w/Java_Edition_protocol/Packets#Cookie_Request_(configuration)
pub fn process_cookie_request_packet(buf: &mut Buf, bot: &mut Bot, compression: &mut Compression) {
    let identifier = buf.read_sized_string();
    bot.send_packet(write_cookie_response(identifier), compression);
}

/// Finish Configuration
/// https://minecraft.wiki/w/Java_Edition_protocol/Packets#Finish_Configuration
pub fn process_finish_configuration(
    _buffer: &mut Buf,
    bot: &mut Bot,
    compression: &mut Compression,
) {
    bot.send_packet(write_acknowledge_configuration(), compression);

    bot.state = ProtocolState::Play;
}

/// Clientbound Keep Alive (configuration)
/// https://minecraft.wiki/w/Java_Edition_protocol/Packets#Clientbound_Keep_Alive_(configuration)
pub fn process_keep_alive_packet(buffer: &mut Buf, bot: &mut Bot, compression: &mut Compression) {
    bot.send_packet(write_keep_alive_packet(buffer.read_u64()), compression);
}

/// Ping (configuration)
/// https://minecraft.wiki/w/Java_Edition_protocol/Packets#Ping_(configuration)
pub fn process_ping(buffer: &mut Buf, bot: &mut Bot, compression: &mut Compression) {
    bot.send_packet(write_pong(buffer.read_u32()), compression);
}

/// Add Resource Pack (configuration)
/// https://minecraft.wiki/w/Java_Edition_protocol/Packets#Add_Resource_Pack_(configuration)
///
/// Confirmed via decompiling Adventure's `ResourcePackStatus` (the enum Minestom uses server-side):
/// ACCEPTED is `intermediate=true`, and Minestom's `Player.onResourcePackStatus` only completes the
/// per-player resource-pack future -- which `ConnectionManager.doConfiguration()` blocks on with an
/// un-timed-out `.join()` before ever sending Finish Configuration -- once a *non-intermediate*
/// status arrives. Sending only ACCEPTED (as this used to) leaves that future pending forever, so
/// the whole connection silently hangs in Configuration state. Real clients always follow ACCEPTED
/// with a terminal status once the pack finishes loading; since this bot never renders anything,
/// immediately report SUCCESSFULLY_LOADED (wire status 0, intermediate=false) right after.
pub fn process_resource_pack(buffer: &mut Buf, bot: &mut Bot, compression: &mut Compression) {
    let id = buffer.read_u128();
    bot.send_packet(write_resource_pack_response(id, 3), compression); // Accepted (intermediate)
    bot.send_packet(write_resource_pack_response(id, 0), compression); // Successfully loaded (terminal)
}

/// Transfer (configuration)
/// https://minecraft.wiki/w/Java_Edition_protocol/Packets#Transfer_(configuration)
pub fn process_transfer(buffer: &mut Buf, bot: &mut Bot, _compression: &mut Compression) {
    // Goofy lifetimes
    let address = buffer.read_sized_string().to_owned();
    let port = buffer.read_u16();

    println!("Server requested transfer to {address}:{port} but it isnt implemented! Please turn off online mode! Disconnecting bot {}", bot.name);
    bot.kicked = true;
}

/// Known Packs (configuration)
/// https://minecraft.wiki/w/Java_Edition_protocol/Packets#Clientbound_Known_Packs
pub fn process_known_packs(_buffer: &mut Buf, bot: &mut Bot, compression: &mut Compression) {
    bot.send_packet(write_known_packets(), compression);
}

/// Code of Conduct (configuration)
///
/// Not part of the classic vanilla protocol -- confirmed via `javap` against this server's
/// exact Minestom build (net.minestom.server.network.packet.server.configuration.CodeOfConductPacket,
/// registry ID 0x13). The server withholds Finish Configuration until this is acknowledged, so a
/// bot that silently ignores it (the framing loop skips unrecognized packets by their declared
/// length with no error) stalls forever after the registry-data burst and never reaches Play.
pub fn process_code_of_conduct(_buffer: &mut Buf, bot: &mut Bot, compression: &mut Compression) {
    bot.send_packet(write_accept_code_of_conduct(), compression);
}

pub fn write_cookie_response(identifier: &str) -> Buf {
    let mut buf = Buf::new();
    buf.write_packet_id(0x01);

    buf.write_sized_str(identifier);
    buf.write_bool(false);

    buf
}

/// Acknowledge Finish Configuration
pub fn write_acknowledge_configuration() -> Buf {
    let mut buf = Buf::new();
    buf.write_packet_id(0x03);

    buf
}

/// Serverbound Keep Alive (configuration)
pub fn write_keep_alive_packet(id: u64) -> Buf {
    // ClientKeepAlivePacket
    let mut buf = Buf::new();
    buf.write_packet_id(0x04);

    buf.write_u64(id);

    buf
}

/// Pong (configuration)
pub fn write_pong(id: u32) -> Buf {
    // ClientKeepAlivePacket
    let mut buf = Buf::new();
    buf.write_packet_id(0x05);

    buf.write_u32(id);

    buf
}

/// Resource Pack Response (configuration)
///
/// `status` is the wire value from Adventure's ResourcePackStatus (confirmed via decompiling
/// Minestom's ClientResourcePackStatusPacket): 0=SUCCESSFULLY_LOADED, 1=DECLINED,
/// 2=FAILED_DOWNLOAD, 3=ACCEPTED, 4=DOWNLOADED, 5=INVALID_URL, 6=FAILED_RELOAD, 7=DISCARDED.
pub fn write_resource_pack_response(id: u128, status: u32) -> Buf {
    let mut buf = Buf::new();
    buf.write_packet_id(0x06);

    buf.write_u128(id);
    buf.write_var_u32(status);

    buf
}

pub fn write_known_packets() -> Buf {
    let mut buf = Buf::new();
    buf.write_packet_id(0x07);

    buf.write_var_u32(0);

    buf
}

/// Accept Code of Conduct (configuration) -- empty payload, just the packet ID.
pub fn write_accept_code_of_conduct() -> Buf {
    let mut buf = Buf::new();
    buf.write_packet_id(0x09);

    buf
}

const VIEW_DISTANCE: u8 = 10u8;

/// Client Information (configuration)
pub fn write_client_settings() -> Buf {
    // ClientSettingsPacket
    let mut buf = Buf::new();
    buf.write_packet_id(0x00);

    // locale
    buf.write_sized_str("en_US");

    // view distance
    buf.write_u8(VIEW_DISTANCE);

    // chat mode
    buf.write_var_u32(0);

    // chat colors
    buf.write_bool(true);

    // skin flags
    buf.write_u8(0xFF);

    // main hand
    buf.write_var_u32(1);

    // enable text filtering
    buf.write_bool(false);

    // allow server listing
    buf.write_bool(true);

    // particle status
    buf.write_var_u32(0);

    buf
}
