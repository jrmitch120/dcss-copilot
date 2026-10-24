// (trimmed for the extractor test)

string article_a(const string &name, bool lowercase)
{
    if (!name.length())
        return name;

    const char *a  = lowercase? "a "  : "A ";
    const char *an = lowercase? "an " : "An ";
    switch (name[0])
    {
        case 'a': case 'e': case 'i': case 'o': case 'u':
        case 'A': case 'E': case 'I': case 'O': case 'U':
            // XXX: Hack for hydras.
            if (starts_with(name, "one-"))
                return a + name;
            return an + name;
        case '1':
            // XXX: Hack^2 for hydras.
            if (starts_with(name, "11-") || starts_with(name, "18-"))
                return an + name;
            return a + name;
        case '8':
            // Eighty, eight hundred, eight thousand, ...
            return an + name;
        default:
            return a + name;
    }
}
