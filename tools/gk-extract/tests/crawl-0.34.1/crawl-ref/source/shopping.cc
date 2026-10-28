// Trimmed from crawl 0.34.1 for the extractor test: a shop's menu.

class ShopEntry : public InvEntry
{
    string get_text() const override
    {
        return make_stringf(" <%s>%c %c </%s><%s>%4d gold   %s%s</%s>",
                            keystr.c_str(),
                            hotkeys[0],
                            selected() ? '+' : on_list ? '$' : '-',
                            keystr.c_str(),
                            itemstr.c_str(),
                            cost,
                            text.c_str(),
                            shop_item_unknown(*item) ? " (unknown)" : "",
                            itemstr.c_str());
    }
};

ShopMenu::ShopMenu(shop_struct& _shop, const level_pos& _pos, bool _can_purchase)
{
    set_tag("shop");

    set_title("Welcome to " + shop_name(shop) + "! What would you "
              "like to do?");
}
