// (trimmed for the extractor test)

bool is_connected_branch(const Branch *branch)
{
    return !testbits(branch->branch_flags, brflag::no_x_level_travel);
}
