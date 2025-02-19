use kovi::build_bot;

fn main() {
    let bot = build_bot!(taro, deepseek);
    bot.run();
}
